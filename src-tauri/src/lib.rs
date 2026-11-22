pub mod discovery;
pub mod media_metadata;
pub mod mpv_controller;
pub mod sync_room;
pub mod transfer;
pub mod updater;

use discovery::{DiscoveryState, LocalDeviceInfo, PeerDevice, DISCOVERY_PORT};
use mpv_controller::{MpvController, MpvPlaybackStatus};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use sync_room::{
    DiscoveredRoom, MovieFileInfo, RoomControlPermissions, RoomMember, SyncRoomManager,
    WatchRoomState,
};
use tauri::{AppHandle, Manager, State};
use transfer::TransferState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub app_name: String,
    pub version: String,
    pub platform: String,
    pub offline_mode: bool,
}

#[tauri::command]
fn get_system_info() -> Result<SystemInfo, String> {
    Ok(SystemInfo {
        app_name: "SyncDrop".to_string(),
        version: "0.1.0".to_string(),
        platform: "windows-x86_64".to_string(),
        offline_mode: true,
    })
}

// ==========================================
// Discovery Commands
// ==========================================

#[tauri::command]
async fn get_discovered_peers(
    state: State<'_, Arc<DiscoveryState>>,
) -> Result<Vec<PeerDevice>, String> {
    Ok(state.get_peers().await)
}

#[tauri::command]
async fn get_local_device_info(
    state: State<'_, Arc<DiscoveryState>>,
) -> Result<LocalDeviceInfo, String> {
    Ok(state.get_local_info().await)
}

#[tauri::command]
async fn set_local_alias(
    alias: String,
    state: State<'_, Arc<DiscoveryState>>,
) -> Result<(), String> {
    state.set_alias(alias).await;
    Ok(())
}

#[tauri::command]
async fn refresh_peer_discovery(
    state: State<'_, Arc<DiscoveryState>>,
) -> Result<Vec<PeerDevice>, String> {
    Ok(state.get_peers().await)
}

#[tauri::command]
fn start_discovery(
    app: AppHandle,
    state: State<'_, Arc<DiscoveryState>>,
) -> Result<(), String> {
    discovery::start_discovery_tasks(app, Arc::clone(&state));
    Ok(())
}

#[tauri::command]
fn stop_discovery(
    state: State<'_, Arc<DiscoveryState>>,
) -> Result<(), String> {
    discovery::stop_discovery_tasks(Arc::clone(&state));
    Ok(())
}

// ==========================================
// File Transfer Commands
// ==========================================

#[tauri::command]
async fn resolve_staged_files(
    paths: Vec<String>,
) -> Result<Vec<transfer::StagedFileInfo>, String> {
    transfer::resolve_staged_files(paths)
}

#[tauri::command]
async fn pick_files_dialog(
    default_dir: Option<String>,
) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || {
        let mut dialog = rfd::FileDialog::new().set_title("Select Files to Send via SyncDrop");
        if let Some(ref dir) = default_dir {
            dialog = dialog.set_directory(dir);
        }
        let paths = dialog.pick_files();
        Ok(paths
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect())
    })
    .await
    .map_err(|e| format!("Dialog thread failed: {}", e))?
}

#[tauri::command]
async fn pick_folder_dialog(
    default_dir: Option<String>,
) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || {
        let mut dialog = rfd::FileDialog::new().set_title("Select Folder to Send via SyncDrop");
        if let Some(ref dir) = default_dir {
            dialog = dialog.set_directory(dir);
        }
        let path = dialog.pick_folder();
        Ok(path.map(|p| p.to_string_lossy().to_string()))
    })
    .await
    .map_err(|e| format!("Dialog thread failed: {}", e))?
}

#[tauri::command]
async fn pick_movie_dialog(
    default_dir: Option<String>,
) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Select Movie File for Watch Party")
            .add_filter(
                "Video Files (*.mp4, *.mkv, *.avi, *.mov, *.webm, etc.)",
                &["mp4", "mkv", "avi", "mov", "webm", "m4v", "ts", "flv", "wmv", "m2ts"],
            )
            .add_filter("All Files (*.*)", &["*"]);

        if let Some(ref dir) = default_dir {
            dialog = dialog.set_directory(dir);
        }
        let path = dialog.pick_file();
        Ok(path.map(|p| p.to_string_lossy().to_string()))
    })
    .await
    .map_err(|e| format!("Dialog thread failed: {}", e))?
}

#[tauri::command]
async fn scan_directory(
    path: Option<String>,
    transfer_state: State<'_, Arc<TransferState>>,
) -> Result<Vec<transfer::StagedFileInfo>, String> {
    let target_dir = match path {
        Some(p) => std::path::PathBuf::from(p),
        None => transfer_state.download_dir.read().await.clone(),
    };
    transfer::scan_directory_files(&target_dir)
}

#[tauri::command]
async fn detect_local_movie(
    filename: String,
    expected_size: u64,
    transfer_state: State<'_, Arc<TransferState>>,
) -> Result<Option<String>, String> {
    let download_dir = transfer_state.download_dir.read().await;
    let dir_str = download_dir.to_string_lossy().to_string();
    Ok(transfer::detect_local_movie(&filename, expected_size, &dir_str))
}

#[tauri::command]
async fn send_files_to_recipients(
    app: AppHandle,
    transfer_state: State<'_, Arc<TransferState>>,
    discovery_state: State<'_, Arc<DiscoveryState>>,
    file_paths: Vec<String>,
    recipient_ids: Vec<String>,
) -> Result<(), String> {
    transfer::send_files_to_recipients(
        app,
        Arc::clone(&transfer_state),
        Arc::clone(&discovery_state),
        file_paths,
        recipient_ids,
    )
    .await
}

#[tauri::command]
async fn respond_to_transfer_prompt(
    session_id: String,
    accept: bool,
    transfer_state: State<'_, Arc<TransferState>>,
) -> Result<(), String> {
    let mut prompts = transfer_state.pending_prompts.write().await;
    if let Some(tx) = prompts.remove(&session_id) {
        let _ = tx.send(accept);
    }
    Ok(())
}

#[tauri::command]
async fn cancel_transfer(
    session_id: String,
    transfer_state: State<'_, Arc<TransferState>>,
) -> Result<(), String> {
    let cancellations = transfer_state.active_cancellations.read().await;
    if let Some(flag) = cancellations.get(&session_id) {
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
async fn get_download_directory(
    transfer_state: State<'_, Arc<TransferState>>,
) -> Result<String, String> {
    let dir = transfer_state.download_dir.read().await;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
async fn set_download_directory(
    path: String,
    transfer_state: State<'_, Arc<TransferState>>,
) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if let Err(e) = tokio::fs::create_dir_all(&p).await {
        return Err(format!("Could not create or access directory: {}", e));
    }
    let mut dir = transfer_state.download_dir.write().await;
    *dir = p;
    Ok(())
}

#[tauri::command]
async fn open_download_folder(
    transfer_state: State<'_, Arc<TransferState>>,
) -> Result<(), String> {
    let dir = transfer_state.download_dir.read().await.clone();
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg(dir)
            .spawn()
            .map_err(|e| format!("Could not open explorer: {}", e))?;
    }
    Ok(())
}

// ==========================================
// mpv Media Engine Commands
// ==========================================

#[tauri::command]
fn check_mpv_binary() -> bool {
    MpvController::is_binary_available()
}

#[tauri::command]
fn get_mpv_binary_path() -> Result<String, String> {
    let path = MpvController::resolve_mpv_binary_path()?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
async fn launch_mpv(
    file_path: Option<String>,
    mpv: State<'_, Arc<MpvController>>,
) -> Result<(), String> {
    mpv.launch(file_path).await
}

#[tauri::command]
async fn play_mpv(mpv: State<'_, Arc<MpvController>>) -> Result<(), String> {
    mpv.play().await
}

#[tauri::command]
async fn pause_mpv(mpv: State<'_, Arc<MpvController>>) -> Result<(), String> {
    mpv.pause().await
}

#[tauri::command]
async fn toggle_pause_mpv(mpv: State<'_, Arc<MpvController>>) -> Result<(), String> {
    mpv.toggle_pause().await
}

#[tauri::command]
async fn seek_mpv(
    seconds: f64,
    absolute: bool,
    mpv: State<'_, Arc<MpvController>>,
) -> Result<(), String> {
    mpv.seek(seconds, absolute).await
}

#[tauri::command]
async fn set_mpv_speed(
    speed: f64,
    mpv: State<'_, Arc<MpvController>>,
) -> Result<(), String> {
    mpv.set_speed(speed).await
}

#[tauri::command]
async fn load_mpv_file(
    path: String,
    mpv: State<'_, Arc<MpvController>>,
) -> Result<(), String> {
    mpv.load_file(&path).await
}

#[tauri::command]
async fn stop_mpv(mpv: State<'_, Arc<MpvController>>) -> Result<(), String> {
    mpv.stop().await
}

#[tauri::command]
async fn get_mpv_status(
    mpv: State<'_, Arc<MpvController>>,
) -> Result<MpvPlaybackStatus, String> {
    Ok(mpv.get_current_status().await)
}

// ==========================================
// Watch Party & Sync Room Commands
// ==========================================

#[tauri::command]
async fn host_watch_room(
    title: String,
    movie_info: Option<MovieFileInfo>,
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<WatchRoomState, String> {
    SyncRoomManager::host_room(Arc::clone(&room_manager), title, movie_info).await
}

#[tauri::command]
async fn join_watch_room(
    host_uri: String,
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<(), String> {
    SyncRoomManager::join_room(Arc::clone(&room_manager), host_uri).await
}

#[tauri::command]
async fn leave_watch_room(
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<(), String> {
    room_manager.leave_room().await
}

#[tauri::command]
async fn get_watch_room_state(
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<WatchRoomState, String> {
    Ok(room_manager.get_current_room_state().await)
}

#[tauri::command]
async fn dispatch_room_play(
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<(), String> {
    room_manager.dispatch_play().await
}

#[tauri::command]
async fn dispatch_room_pause(
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<(), String> {
    room_manager.dispatch_pause().await
}

#[tauri::command]
async fn dispatch_room_seek(
    seconds: f64,
    seek_type: Option<String>,
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<(), String> {
    room_manager.dispatch_seek(seconds, seek_type).await
}

#[tauri::command]
async fn set_room_permissions(
    permissions: RoomControlPermissions,
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<(), String> {
    room_manager.set_permissions(permissions).await;
    Ok(())
}

#[tauri::command]
async fn dispatch_room_speed(
    speed: f64,
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<(), String> {
    room_manager.dispatch_speed(speed).await
}

#[tauri::command]
async fn set_room_movie(
    movie_info: MovieFileInfo,
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<(), String> {
    room_manager.set_movie(movie_info).await;
    Ok(())
}

#[tauri::command]
async fn set_local_watch_movie(
    movie_info: MovieFileInfo,
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<(), String> {
    room_manager.set_local_movie(movie_info).await;
    Ok(())
}

#[tauri::command]
async fn sync_watch_room(
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<(), String> {
    room_manager.request_sync().await;
    Ok(())
}

#[tauri::command]
async fn get_room_members(
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<Vec<RoomMember>, String> {
    let map = room_manager.members.read().await;
    Ok(map.values().cloned().collect())
}

#[tauri::command]
async fn get_discovered_rooms(
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<Vec<DiscoveredRoom>, String> {
    Ok(room_manager.get_discovered_rooms_list().await)
}

#[tauri::command]
async fn inspect_movie_file(
    file_path: String,
    mpv: State<'_, Arc<MpvController>>,
) -> Result<MovieFileInfo, String> {
    let mut info = media_metadata::inspect_local_movie(file_path).await?;
    let st = mpv.get_current_status().await;
    if st.duration > 0.0 {
        info.duration = st.duration;
    }
    info.video_codec = st.video_codec;
    info.audio_codec = st.audio_codec;
    info.resolution = st.resolution;
    Ok(info)
}

#[tauri::command]
async fn get_movie_verification(
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<media_metadata::MovieVerificationResult, String> {
    let host_m = room_manager.current_movie.read().await;
    let local_m = room_manager.local_movie.read().await;
    let approved = room_manager.compatibility_approved.load(std::sync::atomic::Ordering::Relaxed);
    Ok(media_metadata::evaluate_movie_verification(
        host_m.as_ref(),
        local_m.as_ref(),
        approved,
    ))
}

#[tauri::command]
async fn set_compatibility_mode(
    approved: bool,
    room_manager: State<'_, Arc<SyncRoomManager>>,
) -> Result<media_metadata::MovieVerificationResult, String> {
    room_manager.compatibility_approved.store(approved, std::sync::atomic::Ordering::SeqCst);

    // If client is in a room and has local movie, re-execute set_local_movie to trigger late-join loading & catch up!
    if approved {
        let local_m = room_manager.local_movie.read().await.clone();
        if let Some(lm) = local_m {
            let manager_clone = Arc::clone(&room_manager);
            tauri::async_runtime::spawn(async move {
                manager_clone.set_local_movie(lm).await;
            });
        }
    }

    let host_m = room_manager.current_movie.read().await;
    let local_m = room_manager.local_movie.read().await;
    Ok(media_metadata::evaluate_movie_verification(
        host_m.as_ref(),
        local_m.as_ref(),
        approved,
    ))
}

#[tauri::command]
async fn check_for_updates() -> Result<updater::UpdateInfo, String> {
    updater::check_for_updates().await
}

#[tauri::command]
async fn download_and_install_update(
    app: AppHandle,
    download_url: String,
) -> Result<String, String> {
    let path = updater::download_and_install_update(app, download_url).await?;
    Ok(path.to_string_lossy().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let discovery_state = Arc::new(DiscoveryState::new(DISCOVERY_PORT));
    let transfer_state = Arc::new(TransferState::new());
    let mpv_controller = Arc::new(MpvController::new());
    let sync_room_manager = Arc::new(SyncRoomManager::new(
        Arc::clone(&mpv_controller),
        Arc::clone(&discovery_state),
    ));

    let discovery_state_setup = Arc::clone(&discovery_state);
    let transfer_state_setup = Arc::clone(&transfer_state);
    let mpv_controller_setup = Arc::clone(&mpv_controller);
    let sync_room_manager_setup = Arc::clone(&sync_room_manager);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(discovery_state)
        .manage(transfer_state)
        .manage(mpv_controller)
        .manage(sync_room_manager)
        .setup(move |app| {
            // Set AppHandle for notifications
            transfer_state_setup.set_app_handle(app.handle().clone());
            mpv_controller_setup.set_app_handle(app.handle().clone());
            sync_room_manager_setup.set_app_handle(app.handle().clone());

            let ds_clone = Arc::clone(&discovery_state_setup);
            let ts_clone = Arc::clone(&transfer_state_setup);

            // 1. Launch HTTP Transfer Server in async runtime
            tauri::async_runtime::spawn(async move {
                match transfer::start_transfer_http_server(
                    Arc::clone(&ts_clone),
                    transfer::DEFAULT_TRANSFER_PORT,
                )
                .await
                {
                    Ok(bound_port) => {
                        ds_clone.set_port(bound_port);
                        println!("SyncDrop HTTP transfer service active on port: {}", bound_port);
                    }
                    Err(err) => eprintln!("Failed to start HTTP transfer service: {}", err),
                }
            });

            // 2. Automatically launch discovery background tasks on application startup
            discovery::start_discovery_tasks(app.handle().clone(), discovery_state_setup);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let mpv = window.state::<Arc<MpvController>>();
                tauri::async_runtime::block_on(async {
                    let _ = mpv.stop().await;
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_system_info,
            get_discovered_peers,
            get_local_device_info,
            set_local_alias,
            refresh_peer_discovery,
            start_discovery,
            stop_discovery,
            resolve_staged_files,
            pick_files_dialog,
            pick_folder_dialog,
            pick_movie_dialog,
            scan_directory,
            detect_local_movie,
            send_files_to_recipients,
            respond_to_transfer_prompt,
            cancel_transfer,
            get_download_directory,
            set_download_directory,
            open_download_folder,
            check_mpv_binary,
            get_mpv_binary_path,
            launch_mpv,
            play_mpv,
            pause_mpv,
            toggle_pause_mpv,
            seek_mpv,
            set_mpv_speed,
            load_mpv_file,
            stop_mpv,
            get_mpv_status,
            host_watch_room,
            join_watch_room,
            leave_watch_room,
            get_watch_room_state,
            dispatch_room_play,
            dispatch_room_pause,
            dispatch_room_seek,
            dispatch_room_speed,
            set_room_movie,
            set_local_watch_movie,
            sync_watch_room,
            set_room_permissions,
            get_room_members,
            get_discovered_rooms,
            inspect_movie_file,
            get_movie_verification,
            set_compatibility_mode,
            check_for_updates,
            download_and_install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SyncDrop desktop application");
}
