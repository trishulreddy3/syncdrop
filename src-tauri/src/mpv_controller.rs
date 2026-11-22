//! mpv Media Engine Controller (Process Lifecycle & Named Pipe JSON IPC)
//! Specifications: PROJECT_SPEC.md Section 2.3 & Phase 5
//! - Tauri v2 Sidecar packaging and dynamic path resolution
//! - Managed child process with reliable graceful shutdown and force-kill cleanup
//! - Windows Named Pipe JSON IPC communication (`\\.\pipe\syncdrop_mpv_<session>`)
//! - Non-blocking property observation (`time-pos`, `duration`, `pause`, `speed`, `filename`)
//! - Play, Pause, Seek (absolute/relative), Speed adjustment, and File loading
//! - Automatic reconnection, unexpected exit handling, and live Tauri event broadcasting

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
use tokio::process::Command;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

pub const SIDECAR_NAME: &str = "mpv";
pub const SIDECAR_EXPECTED_FILENAME: &str = "mpv-x86_64-pc-windows-msvc.exe";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MpvPlaybackStatus {
    pub engine_status: String, // "uninitialized" | "starting" | "ready" | "loading" | "playing" | "paused" | "stopped" | "error"
    pub time_pos: f64,         // Current playback position in seconds
    pub duration: f64,         // Total duration in seconds
    pub pause: bool,           // True if paused
    pub speed: f64,            // Current playback speed multiplier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl Default for MpvPlaybackStatus {
    fn default() -> Self {
        Self {
            engine_status: "uninitialized".to_string(),
            time_pos: 0.0,
            duration: 0.0,
            pause: true,
            speed: 1.0,
            filename: None,
            video_codec: None,
            audio_codec: None,
            resolution: None,
            error_message: None,
        }
    }
}

pub struct MpvController {
    pub app_handle: Arc<std::sync::RwLock<Option<AppHandle>>>,
    pub status: Arc<RwLock<MpvPlaybackStatus>>,
    pub child_process: Arc<RwLock<Option<tokio::process::Child>>>,
    pub ipc_writer: Arc<RwLock<Option<tokio::io::WriteHalf<NamedPipeClient>>>>,
    pub pipe_name: Arc<RwLock<Option<String>>>,
    pub is_stopping: Arc<AtomicBool>,
    pub property_tx: broadcast::Sender<(String, serde_json::Value)>,
}

impl MpvController {
    pub fn new() -> Self {
        let (property_tx, _) = broadcast::channel(128);
        Self {
            app_handle: Arc::new(std::sync::RwLock::new(None)),
            status: Arc::new(RwLock::new(MpvPlaybackStatus::default())),
            child_process: Arc::new(RwLock::new(None)),
            ipc_writer: Arc::new(RwLock::new(None)),
            pipe_name: Arc::new(RwLock::new(None)),
            is_stopping: Arc::new(AtomicBool::new(false)),
            property_tx,
        }
    }

    pub fn subscribe_properties(&self) -> broadcast::Receiver<(String, serde_json::Value)> {
        self.property_tx.subscribe()
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        if let Ok(mut lock) = self.app_handle.write() {
            *lock = Some(handle);
        }
    }

    fn emit_status(&self, status: &MpvPlaybackStatus) {
        if let Ok(lock) = self.app_handle.read() {
            if let Some(handle) = lock.as_ref() {
                let _ = handle.emit("syncdrop://mpv-status-changed", status);
            }
        }
    }

    /// Resolves the mpv binary location using Tauri's sidecar paths and local binaries
    pub fn resolve_mpv_binary_path() -> Result<PathBuf, String> {
        let mut searched_paths = Vec::new();

        // 1. Check relative to current exe directory (production NSIS installation)
        if let Ok(curr_exe) = std::env::current_exe() {
            if let Some(parent) = curr_exe.parent() {
                let p1 = parent.join(SIDECAR_EXPECTED_FILENAME);
                if p1.is_file() {
                    return Ok(p1);
                }
                searched_paths.push(p1);

                let p2 = parent.join("mpv.exe");
                if p2.is_file() {
                    return Ok(p2);
                }
                searched_paths.push(p2);

                let p3 = parent.join("binaries").join(SIDECAR_EXPECTED_FILENAME);
                if p3.is_file() {
                    return Ok(p3);
                }
                searched_paths.push(p3);
            }
        }

        // 2. Check development workspace paths
        let dev_paths = [
            PathBuf::from("src-tauri/binaries").join(SIDECAR_EXPECTED_FILENAME),
            PathBuf::from("binaries").join(SIDECAR_EXPECTED_FILENAME),
            PathBuf::from("../src-tauri/binaries").join(SIDECAR_EXPECTED_FILENAME),
            PathBuf::from("src-tauri/binaries/mpv.exe"),
            PathBuf::from("binaries/mpv.exe"),
        ];

        for p in &dev_paths {
            if p.is_file() {
                return Ok(p.canonicalize().unwrap_or_else(|_| p.clone()));
            }
            searched_paths.push(p.clone());
        }

        // 3. Fallback check for system mpv
        if let Ok(output) = std::process::Command::new("where.exe").arg("mpv").output() {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = path_str.lines().next() {
                    let sys_path = PathBuf::from(first_line.trim());
                    if sys_path.is_file() {
                        return Ok(sys_path);
                    }
                }
            }
        }

        let paths_formatted = searched_paths
            .iter()
            .map(|p| format!("  - {}", p.display()))
            .collect::<Vec<_>>()
            .join("\n");

        Err(format!(
            "Bundled mpv executable not found.\nSearched locations:\n{}\nPlease verify mpv-x86_64-pc-windows-msvc.exe is placed in src-tauri/binaries/ (see PROJECT_SPEC.md Section 2.3).",
            paths_formatted
        ))
    }

    /// Verifies whether mpv binary exists on the device
    pub fn is_binary_available() -> bool {
        Self::resolve_mpv_binary_path().is_ok()
    }

    /// Launches mpv sidecar and connects to its named pipe JSON IPC server
    pub async fn launch(&self, initial_file: Option<String>) -> Result<(), String> {
        // If mpv is already running and connected to IPC, load file directly instead of killing and recreating process
        {
            let cp_lock = self.child_process.read().await;
            let writer_lock = self.ipc_writer.read().await;
            if cp_lock.is_some() && writer_lock.is_some() {
                drop(cp_lock);
                drop(writer_lock);
                if let Some(ref file_path) = initial_file {
                    self.send_command(vec![
                        serde_json::Value::String("loadfile".into()),
                        serde_json::Value::String(file_path.clone().into()),
                        serde_json::Value::String("replace".into()),
                    ])
                    .await?;
                    let _ = self.pause().await;
                }
                return Ok(());
            }
        }

        // Clean up any previously running instance
        self.stop().await?;
        self.is_stopping.store(false, Ordering::SeqCst);

        // Update status to starting
        {
            let mut st = self.status.write().await;
            st.engine_status = "starting".to_string();
            st.error_message = None;
            self.emit_status(&st);
        }

        let mpv_binary = Self::resolve_mpv_binary_path()?;
        println!("Launching mpv media engine from: {}", mpv_binary.display());

        // Generate unique Windows named pipe path
        let session_id = Uuid::new_v4().to_string();
        let pipe_id = format!("syncdrop_mpv_{}", &session_id[..8]);
        let pipe_path = format!(r"\\.\pipe\{}", pipe_id);
        {
            let mut p = self.pipe_name.write().await;
            *p = Some(pipe_path.clone());
        }

        // Configure mpv command line arguments (start paused to prevent unauthorized playback from 00:00)
        let mut cmd = Command::new(&mpv_binary);
        cmd.arg("--idle=yes")
            .arg(format!("--input-ipc-server={}", pipe_path))
            .arg("--force-window=immediate")
            .arg("--keep-open=yes")
            .arg("--pause=yes")
            .arg("--hr-seek=yes")
            .arg("--hr-seek-framedrop=no")
            .arg("--audio-pitch-correction=yes")
            .arg("--audio-buffer=0.5")
            .arg("--audio-stream-silence=yes")
            .arg("--video-sync=audio")
            .arg("--title=SyncDrop Movie Player")
            .arg("--autofit=75%")
            .arg("--geometry=50%:50%");

        if let Some(ref file_path) = initial_file {
            cmd.arg(file_path);
        }

        // Spawn child process
        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(err) => {
                let err_msg = format!("Failed to launch mpv sidecar process: {}", err);
                let mut st = self.status.write().await;
                st.engine_status = "error".to_string();
                st.error_message = Some(err_msg.clone());
                self.emit_status(&st);
                return Err(err_msg);
            }
        };

        {
            let mut cp = self.child_process.write().await;
            *cp = Some(child);
        }

        // Connect to named pipe with retries (polling up to 4 seconds)
        let mut client: Option<NamedPipeClient> = None;
        let start_time = tokio::time::Instant::now();
        let timeout = Duration::from_secs(4);

        while start_time.elapsed() < timeout {
            match ClientOptions::new().open(&pipe_path) {
                Ok(c) => {
                    client = Some(c);
                    break;
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(60)).await;
                }
            }
        }

        let pipe_client = match client {
            Some(c) => c,
            None => {
                // Pipe failed to become available
                let err_msg = format!(
                    "Timed out waiting for mpv IPC named pipe server at {}",
                    pipe_path
                );
                let mut st = self.status.write().await;
                st.engine_status = "error".to_string();
                st.error_message = Some(err_msg.clone());
                self.emit_status(&st);
                return Err(err_msg);
            }
        };

        println!("Connected to mpv JSON IPC named pipe: {}", pipe_path);

        // Split pipe into read and write halves
        let (read_half, mut write_half) = tokio::io::split(pipe_client);

        // Send observe_property commands to mpv so it reports events automatically
        let observe_commands = [
            r#"{"command":["observe_property",1,"time-pos"]}"#,
            r#"{"command":["observe_property",2,"pause"]}"#,
            r#"{"command":["observe_property",3,"duration"]}"#,
            r#"{"command":["observe_property",4,"speed"]}"#,
            r#"{"command":["observe_property",5,"filename"]}"#,
            r#"{"command":["observe_property",6,"video-format"]}"#,
            r#"{"command":["observe_property",7,"audio-codec-name"]}"#,
            r#"{"command":["observe_property",8,"width"]}"#,
            r#"{"command":["observe_property",9,"height"]}"#,
        ];

        for cmd_str in &observe_commands {
            let data = format!("{}\n", cmd_str);
            let _ = write_half.write_all(data.as_bytes()).await;
        }

        {
            let mut writer_lock = self.ipc_writer.write().await;
            *writer_lock = Some(write_half);
        }

        // Update initial status
        {
            let mut st = self.status.write().await;
            st.engine_status = if initial_file.is_some() {
                "loading".to_string()
            } else {
                "ready".to_string()
            };
            self.emit_status(&st);
        }

        // Spawn background task to observe property changes and handle exit
        let status_arc = Arc::clone(&self.status);
        let app_handle_arc = Arc::clone(&self.app_handle);
        let child_arc = Arc::clone(&self.child_process);
        let stopping_arc = Arc::clone(&self.is_stopping);
        let property_tx_clone = self.property_tx.clone();

        tauri::async_runtime::spawn(async move {
            let mut reader = BufReader::new(read_half).lines();

            while let Ok(Some(line)) = reader.next_line().await {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                    // Check for property-change events
                    if val.get("event") == Some(&serde_json::Value::String("property-change".into())) {
                        let prop = val.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let data = val.get("data");

                        let mut st = status_arc.write().await;
                        match prop {
                            "time-pos" => {
                                if let Some(secs) = data.and_then(|d| d.as_f64()) {
                                    st.time_pos = secs;
                                }
                            }
                            "duration" => {
                                if let Some(secs) = data.and_then(|d| d.as_f64()) {
                                    st.duration = secs;
                                }
                            }
                            "pause" => {
                                if let Some(is_paused) = data.and_then(|d| d.as_bool()) {
                                    st.pause = is_paused;
                                    st.engine_status = if is_paused {
                                        "paused".to_string()
                                    } else {
                                        "playing".to_string()
                                    };
                                }
                            }
                            "speed" => {
                                if let Some(spd) = data.and_then(|d| d.as_f64()) {
                                    st.speed = spd;
                                }
                            }
                            "filename" => {
                                if let Some(f) = data.and_then(|d| d.as_str()) {
                                    st.filename = Some(f.to_string());
                                }
                            }
                            "video-format" => {
                                if let Some(codec) = data.and_then(|d| d.as_str()) {
                                    st.video_codec = Some(codec.to_string());
                                }
                            }
                            "audio-codec-name" => {
                                if let Some(codec) = data.and_then(|d| d.as_str()) {
                                    st.audio_codec = Some(codec.to_string());
                                }
                            }
                            "width" => {
                                if let Some(w) = data.and_then(|d| d.as_i64()) {
                                    let current_h = st.resolution.as_ref().and_then(|r| r.split('x').nth(1)).unwrap_or("?");
                                    st.resolution = Some(format!("{}x{}", w, current_h));
                                }
                            }
                            "height" => {
                                if let Some(h) = data.and_then(|d| d.as_i64()) {
                                    let current_w = st.resolution.as_ref().and_then(|r| r.split('x').next()).unwrap_or("?");
                                    st.resolution = Some(format!("{}x{}", current_w, h));
                                }
                            }
                            _ => {}
                        }

                        // Notify internal subscribers (e.g. SyncRoomManager for instant spacebar detection)
                        let _ = property_tx_clone.send((prop.to_string(), data.cloned().unwrap_or(serde_json::Value::Null)));

                        // Broadcast update
                        if let Ok(lock) = app_handle_arc.read() {
                            if let Some(h) = lock.as_ref() {
                                let _ = h.emit("syncdrop://mpv-status-changed", &*st);
                            }
                        }
                    }
                }
            }

            // Reader exited -> pipe closed or mpv shut down
            println!("mpv IPC reader loop terminated.");

            let was_intentional = stopping_arc.load(Ordering::SeqCst);
            {
                let mut st = status_arc.write().await;
                st.engine_status = if was_intentional {
                    "stopped".to_string()
                } else {
                    "stopped".to_string()
                };
                if let Ok(lock) = app_handle_arc.read() {
                    if let Some(h) = lock.as_ref() {
                        let _ = h.emit("syncdrop://mpv-status-changed", &*st);
                    }
                }
            }

            // Ensure child process is cleaned up
            let mut cp_lock = child_arc.write().await;
            if let Some(ref mut child) = *cp_lock {
                let _ = child.kill().await;
            }
            *cp_lock = None;
        });

        Ok(())
    }

    /// Sends a raw JSON IPC command to mpv over the active named pipe
    pub async fn send_command(&self, command: Vec<serde_json::Value>) -> Result<(), String> {
        let mut writer_lock = self.ipc_writer.write().await;
        if let Some(ref mut writer) = *writer_lock {
            let payload = serde_json::json!({
                "command": command
            });
            let msg = format!("{}\n", payload);
            writer
                .write_all(msg.as_bytes())
                .await
                .map_err(|e| format!("Failed to write to mpv IPC: {}", e))?;
            writer
                .flush()
                .await
                .map_err(|e| format!("Failed to flush mpv IPC: {}", e))?;
            Ok(())
        } else {
            Err("mpv IPC connection is not active".to_string())
        }
    }

    /// Resumes playback
    pub async fn play(&self) -> Result<(), String> {
        self.send_command(vec![
            serde_json::Value::String("set_property".into()),
            serde_json::Value::String("pause".into()),
            serde_json::Value::Bool(false),
        ])
        .await
    }

    /// Pauses playback
    pub async fn pause(&self) -> Result<(), String> {
        self.send_command(vec![
            serde_json::Value::String("set_property".into()),
            serde_json::Value::String("pause".into()),
            serde_json::Value::Bool(true),
        ])
        .await
    }

    /// Toggles pause state
    pub async fn toggle_pause(&self) -> Result<(), String> {
        self.send_command(vec![
            serde_json::Value::String("cycle".into()),
            serde_json::Value::String("pause".into()),
        ])
        .await
    }

    /// Seeks to a specific position in seconds
    pub async fn seek(&self, seconds: f64, absolute: bool) -> Result<(), String> {
        let mode = if absolute { "absolute" } else { "relative" };
        self.send_command(vec![
            serde_json::Value::String("seek".into()),
            serde_json::json!(seconds),
            serde_json::Value::String(mode.into()),
        ])
        .await
    }

    /// Adjusts playback speed multiplier (e.g. 1.0, 1.05, 0.95)
    pub async fn set_speed(&self, speed: f64) -> Result<(), String> {
        self.send_command(vec![
            serde_json::Value::String("set_property".into()),
            serde_json::Value::String("speed".into()),
            serde_json::json!(speed),
        ])
        .await
    }

    /// Loads a movie file into the running player in paused state
    pub async fn load_file(&self, file_path: &str) -> Result<(), String> {
        // If mpv is not running yet, launch it with this file (launch already enforces --pause=yes)
        {
            let cp = self.child_process.read().await;
            let writer = self.ipc_writer.read().await;
            if cp.is_none() || writer.is_none() {
                drop(cp);
                drop(writer);
                return self.launch(Some(file_path.to_string())).await;
            }
        }

        self.send_command(vec![
            serde_json::Value::String("loadfile".into()),
            serde_json::Value::String(file_path.into()),
            serde_json::Value::String("replace".into()),
        ])
        .await?;

        // Keep player paused upon loading to prevent automatic playback before room synchronization
        let _ = self.pause().await;

        let mut st = self.status.write().await;
        st.filename = Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());
        st.engine_status = "loading".to_string();
        self.emit_status(&st);

        Ok(())
    }

    /// Stops playback and closes active file
    pub async fn stop_playback(&self) -> Result<(), String> {
        self.send_command(vec![serde_json::Value::String("stop".into())])
            .await
    }

    /// Gracefully closes mpv and cleans up process and IPC handles
    pub async fn stop(&self) -> Result<(), String> {
        self.is_stopping.store(true, Ordering::SeqCst);

        // Try graceful quit via IPC
        let _ = self
            .send_command(vec![serde_json::Value::String("quit".into())])
            .await;

        // Close IPC writer
        {
            let mut writer = self.ipc_writer.write().await;
            *writer = None;
        }

        // Wait or kill child process
        let mut cp = self.child_process.write().await;
        if let Some(mut child) = cp.take() {
            // Give child 500ms to exit gracefully
            let _ = tokio::time::timeout(Duration::from_millis(500), child.wait()).await;
            let _ = child.kill().await;
        }

        let mut st = self.status.write().await;
        st.engine_status = "stopped".to_string();
        self.emit_status(&st);

        Ok(())
    }

    /// Returns current in-memory status snapshot
    pub async fn get_current_status(&self) -> MpvPlaybackStatus {
        self.status.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_mpv_binary() {
        let res = MpvController::resolve_mpv_binary_path();
        assert!(res.is_ok(), "Expected mpv binary to be resolved: {:?}", res);
        let path = res.unwrap();
        assert!(path.is_file(), "Resolved path is not a file: {}", path.display());
    }

    #[test]
    fn test_mpv_status_default() {
        let st = MpvPlaybackStatus::default();
        assert_eq!(st.engine_status, "uninitialized");
        assert_eq!(st.speed, 1.0);
        assert!(st.pause);
    }
}
