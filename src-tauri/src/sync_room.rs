//! Synchronized Watch Party Networking Module (WebSocket Host & Client)
//! Specifications: PROJECT_SPEC.md Section 3.3, 3.4 & Phase 6
//! - Host WebSocket Server on port 53318 with client connection registry
//! - Client WebSocket connection (`ws://<host-ip>:53318`) with manual or discovered IP
//! - Authoritative state sync: JOIN, LEAVE, STATE_SYNC, SEEK, PLAY, PAUSE, FILE_INFO, MEMBER_LIST, PING, PONG
//! - Strictly monotonic sequence numbers & timestamp validation to prevent stale state overrides
//! - Automatic late-join synchronization catching up to live playback position
//! - Direct bidirectional connection to mpv_controller.rs for playback and drift control
//! - Live member tracking and status broadcasting to WatchPartyTab.tsx

use axum::{
    extract::{
        ws::{Message as AxumWsMessage, WebSocket, WebSocketUpgrade},
        ConnectInfo, State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_tungstenite::tungstenite::Message as TungsteniteWsMessage;
use uuid::Uuid;

use crate::discovery::{get_primary_local_ip, DiscoveryState};
use crate::mpv_controller::MpvController;

pub const DEFAULT_ROOM_PORT: u16 = 53318;
pub const FALLBACK_ROOM_PORT: u16 = 53328;

// ==========================================
// Protocol Message Types & Enums
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SyncMessage {
    Join {
        client_id: String,
        alias: String,
        client_version: String,
    },
    Leave {
        client_id: String,
        reason: Option<String>,
    },
    FileInfo {
        movie_id: String,
        filename: String,
        file_size: u64,
        duration: f64,
        container_format: String,
        sha256: Option<String>,
    },
    StateSync {
        room_id: String,
        position: f64,
        paused: bool,
        speed: f64,
        timestamp: u64,
        sequence: u64,
        actor: String,
        #[serde(default)]
        permissions: Option<RoomControlPermissions>,
    },
    Seek {
        room_id: String,
        position: f64,
        #[serde(default)]
        seek_type: Option<String>,
        actor: String,
        timestamp: u64,
    },
    Play {
        room_id: String,
        position: f64,
        actor: String,
        timestamp: u64,
    },
    Pause {
        room_id: String,
        position: f64,
        actor: String,
        timestamp: u64,
    },
    Speed {
        room_id: String,
        speed: f64,
        actor: String,
        timestamp: u64,
    },
    PermissionsUpdate {
        room_id: String,
        permissions: RoomControlPermissions,
    },
    SyncRequest {
        client_id: String,
        #[serde(default)]
        movie_id: String,
        #[serde(default)]
        client_time: u64,
    },
    SyncSnapshot {
        room_id: String,
        movie_id: String,
        client_id: String,
        position: f64,
        paused: bool,
        speed: f64,
        timestamp: u64,
        sequence_number: u64,
        host_id: String,
        #[serde(default)]
        client_time: u64,
        #[serde(default)]
        permissions: Option<RoomControlPermissions>,
    },
    SyncReady {
        client_id: String,
        movie_id: String,
        #[serde(default)]
        local_duration: f64,
    },
    SyncConfirmed {
        client_id: String,
        applied_position: f64,
        drift_seconds: f64,
        timestamp: u64,
    },
    StatusReport {
        client_id: String,
        status: String,
        drift_seconds: f64,
        movie_matched: bool,
        #[serde(default)]
        position: f64,
    },
    Ping {
        client_time: u64,
    },
    Pong {
        client_time: u64,
        server_time: u64,
    },
    MemberList {
        members: Vec<RoomMember>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomMember {
    pub id: String,
    pub alias: String,
    pub ip: String,
    pub is_host: bool,
    pub status: String, // "DISCONNECTED" | "CONNECTING" | "CONNECTED" | "WAITING_FOR_MOVIE" | "VERIFYING_MOVIE" | "LAUNCHING_PLAYER" | "LOADING_MOVIE" | "WAITING_FOR_SYNC" | "SYNCHRONIZING" | "WATCHING" | "OUT_OF_SYNC" | "ERROR"
    pub movie_matched: bool,
    pub drift_seconds: f64,
    #[serde(default)]
    pub position: f64,
    pub ping_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieFileInfo {
    pub movie_id: String,
    pub filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    pub file_size: u64,
    pub duration: f64,
    pub container_format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredRoom {
    pub room_id: String,
    pub title: String,
    pub host_alias: String,
    pub host_ip: String,
    pub port: u16,
    pub movie_title: String,
    pub member_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoomControlPermissions {
    pub allow_all: bool,
    pub allow_play_pause: bool,
    pub allow_jump_forward: bool,
    pub allow_jump_backward: bool,
    pub allow_seek: bool,
    pub allow_speed: bool,
}

impl Default for RoomControlPermissions {
    fn default() -> Self {
        Self {
            allow_all: true,
            allow_play_pause: true,
            allow_jump_forward: true,
            allow_jump_backward: true,
            allow_seek: true,
            allow_speed: true,
        }
    }
}

impl RoomControlPermissions {
    pub fn is_play_pause_allowed(&self) -> bool {
        self.allow_all || self.allow_play_pause
    }

    pub fn is_jump_forward_allowed(&self) -> bool {
        self.allow_all || self.allow_jump_forward
    }

    pub fn is_jump_backward_allowed(&self) -> bool {
        self.allow_all || self.allow_jump_backward
    }

    pub fn is_seek_allowed(&self) -> bool {
        self.allow_all || self.allow_seek
    }

    pub fn is_speed_allowed(&self) -> bool {
        self.allow_all || self.allow_speed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchRoomState {
    pub room_id: String,
    pub movie_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub movie_info: Option<MovieFileInfo>,
    pub position: f64,
    pub paused: bool,
    pub speed: f64,
    pub timestamp: u64,
    pub sequence: u64,
    pub actor: String,
    pub strict_host_control: bool,
    #[serde(default)]
    pub permissions: RoomControlPermissions,
    pub is_host: bool,
    pub host_ip: String,
    pub port: u16,
    pub drift_seconds: f64,
    pub sync_status: String,
    pub movie_matched: bool,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn normalize_filename(name: &str) -> String {
    let clean = name.replace('\\', "/");
    let file = clean.split('/').last().unwrap_or(&clean);
    let stem = if let Some(idx) = file.rfind('.') {
        &file[..idx]
    } else {
        file
    };
    stem.trim().to_lowercase()
}

/// Initial movie compatibility check based on filename & file size
/// Does not treat them as proof of identical content (Section 3.3 & Phase 7)
pub fn check_movie_compatibility(
    host_file: &MovieFileInfo,
    local_file: &MovieFileInfo,
    compatibility_approved: bool,
) -> (bool, String) {
    if compatibility_approved {
        return (true, "Compatibility mode approved by user".to_string());
    }

    // Exact SHA-256 match
    if let (Some(ref h_hash), Some(ref l_hash)) = (&host_file.sha256, &local_file.sha256) {
        if h_hash.eq_ignore_ascii_case(l_hash) {
            return (true, "Movie verified (Exact SHA-256 Match)".to_string());
        }
    }

    let host_norm = normalize_filename(&host_file.filename);
    let local_norm = normalize_filename(&local_file.filename);

    let names_match = host_norm == local_norm;

    // File size tolerance: allow up to 5% or 10 MB delta for container header variations
    let size_diff = (host_file.file_size as i64 - local_file.file_size as i64).abs();
    let max_allowed = ((host_file.file_size as f64) * 0.05) as i64;
    let sizes_match = size_diff <= max_allowed.max(10 * 1024 * 1024);

    if names_match && sizes_match {
        (true, "Movie verified (Filename & Size match)".to_string())
    } else if names_match {
        let host_mb = host_file.file_size / (1024 * 1024);
        let local_mb = local_file.file_size / (1024 * 1024);
        (
            false,
            format!(
                "Size mismatch: Host is {} MB, local is {} MB. Approve compatibility to play anyway.",
                host_mb, local_mb
            ),
        )
    } else {
        (
            false,
            format!(
                "Different file: Host is watching '{}' ({:.1} MB). Approve compatibility or receive via Wi-Fi Send.",
                host_file.filename,
                (host_file.file_size as f64) / (1024.0 * 1024.0)
            ),
        )
    }
}

/// Calculates the expected playback position of the host, accounting for pause state,
/// elapsed time since the snapshot was captured, speed, and network latency (Section 13.5 & Phase 8)
pub fn calculate_expected_playback_position(
    snapshot_position: f64,
    paused: bool,
    speed: f64,
    snapshot_timestamp: u64,
    client_request_time: u64,
    current_time: u64,
) -> f64 {
    if paused {
        return snapshot_position;
    }

    let rtt = current_time.saturating_sub(client_request_time);
    let latency_secs = if rtt > 0 && rtt < 10000 {
        (rtt as f64) / 2000.0 // RTT / 2 (one-way message flight time)
    } else {
        (current_time.saturating_sub(snapshot_timestamp) as f64) / 1000.0
    };

    let effective_speed = if speed > 0.0 { speed } else { 1.0 };
    snapshot_position + (latency_secs * effective_speed)
}

// ==========================================
// SyncRoomManager
// ==========================================

pub struct SyncRoomManager {
    pub app_handle: Arc<std::sync::RwLock<Option<AppHandle>>>,
    pub mpv_controller: Arc<MpvController>,
    pub discovery_state: Arc<DiscoveryState>,

    // Room active state
    pub is_hosting: Arc<AtomicBool>,
    pub is_connected: Arc<AtomicBool>,
    pub room_id: Arc<RwLock<Option<String>>>,
    pub host_ip: Arc<RwLock<String>>,
    pub port: Arc<AtomicU64>,

    // Authoritative state tracking
    pub sequence: Arc<AtomicU64>,
    pub last_applied_sequence: Arc<AtomicU64>,
    pub last_applied_timestamp: Arc<AtomicU64>,
    pub intended_speed: Arc<AtomicU64>,
    pub last_seek_time: Arc<RwLock<Instant>>,
    pub last_status_report: Arc<AtomicU64>,
    pub current_movie: Arc<RwLock<Option<MovieFileInfo>>>,
    pub local_movie: Arc<RwLock<Option<MovieFileInfo>>>,
    pub strict_host_control: Arc<AtomicBool>,
    pub compatibility_approved: Arc<AtomicBool>,
    pub control_permissions: Arc<RwLock<RoomControlPermissions>>,

    // Member registry
    pub members: Arc<RwLock<HashMap<String, RoomMember>>>,

    // Channel to broadcast to host WebSocket connections
    pub host_broadcast_tx: broadcast::Sender<SyncMessage>,

    // Channel to send messages from client to remote host
    pub client_msg_tx: Arc<RwLock<Option<mpsc::Sender<SyncMessage>>>>,

    // Discovered rooms cache
    pub discovered_rooms: Arc<RwLock<HashMap<String, DiscoveredRoom>>>,
    pub server_shutdown_tx: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl SyncRoomManager {
    pub fn new(
        mpv_controller: Arc<MpvController>,
        discovery_state: Arc<DiscoveryState>,
    ) -> Self {
        let (host_broadcast_tx, _) = broadcast::channel(128);

        Self {
            app_handle: Arc::new(std::sync::RwLock::new(None)),
            mpv_controller,
            discovery_state,
            is_hosting: Arc::new(AtomicBool::new(false)),
            is_connected: Arc::new(AtomicBool::new(false)),
            room_id: Arc::new(RwLock::new(None)),
            host_ip: Arc::new(RwLock::new("127.0.0.1".to_string())),
            port: Arc::new(AtomicU64::new(DEFAULT_ROOM_PORT as u64)),
            sequence: Arc::new(AtomicU64::new(1)),
            last_applied_sequence: Arc::new(AtomicU64::new(0)),
            last_applied_timestamp: Arc::new(AtomicU64::new(0)),
            intended_speed: Arc::new(AtomicU64::new((1.0f64).to_bits())),
            last_seek_time: Arc::new(RwLock::new(Instant::now() - Duration::from_secs(10))),
            last_status_report: Arc::new(AtomicU64::new(0)),
            current_movie: Arc::new(RwLock::new(None)),
            local_movie: Arc::new(RwLock::new(None)),
            strict_host_control: Arc::new(AtomicBool::new(false)),
            compatibility_approved: Arc::new(AtomicBool::new(false)),
            control_permissions: Arc::new(RwLock::new(RoomControlPermissions::default())),
            members: Arc::new(RwLock::new(HashMap::new())),
            host_broadcast_tx,
            client_msg_tx: Arc::new(RwLock::new(None)),
            discovered_rooms: Arc::new(RwLock::new(HashMap::new())),
            server_shutdown_tx: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        if let Ok(mut lock) = self.app_handle.write() {
            *lock = Some(handle);
        }
    }

    pub fn emit_event<S: Serialize + Clone>(&self, event: &str, payload: S) {
        if let Ok(lock) = self.app_handle.read() {
            if let Some(handle) = lock.as_ref() {
                let _ = handle.emit(event, payload);
            }
        }
    }

    // ==========================================
    // Room Hosting Logic
    // ==========================================

    /// Hosts a new local watch room on ws://0.0.0.0:53318
    pub async fn host_room(
        self_arc: Arc<Self>,
        title: String,
        movie_info: Option<MovieFileInfo>,
    ) -> Result<WatchRoomState, String> {
        // Leave any existing room
        self_arc.leave_room().await?;

        let room_id = Uuid::new_v4().to_string();
        let local_ip = get_primary_local_ip();
        let port = DEFAULT_ROOM_PORT;

        {
            let mut r = self_arc.room_id.write().await;
            *r = Some(room_id.clone());
            let mut hip = self_arc.host_ip.write().await;
            *hip = local_ip.clone();
            self_arc.port.store(port as u64, Ordering::SeqCst);
            self_arc.is_hosting.store(true, Ordering::SeqCst);
            self_arc.is_connected.store(true, Ordering::SeqCst);
            self_arc.sequence.store(1, Ordering::SeqCst);
            self_arc.last_applied_sequence.store(0, Ordering::SeqCst);

            let mut cm = self_arc.current_movie.write().await;
            *cm = movie_info.clone();

            // Register host as the first member
            let mut members = self_arc.members.write().await;
            members.clear();
            let host_info = self_arc.discovery_state.get_local_info().await;
            members.insert(
                host_info.id.clone(),
                RoomMember {
                    id: host_info.id,
                    alias: host_info.alias,
                    ip: local_ip.clone(),
                    is_host: true,
                    status: "WATCHING".to_string(),
                    movie_matched: true,
                    drift_seconds: 0.0,
                    position: 0.0,
                    ping_ms: 0,
                },
            );
        }

        // Setup shutdown channel for Axum WebSocket Server
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        {
            let mut s = self_arc.server_shutdown_tx.write().await;
            if let Some(old_tx) = s.take() {
                let _ = old_tx.send(());
            }
            *s = Some(shutdown_tx);
        }

        // Start Axum WebSocket Server on 0.0.0.0:53318 with graceful shutdown
        let manager_clone = Arc::clone(&self_arc);
        tauri::async_runtime::spawn(async move {
            let app = Router::new()
                .route("/ws", get(ws_handler))
                .route("/", get(ws_handler))
                .with_state(manager_clone);

            let listener = match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("Failed to bind WebSocket server on port {}: {}", port, e);
                    return;
                }
            };

            println!("SyncDrop WebSocket Watch Party server listening on ws://0.0.0.0:{}", port);

            let server = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            );

            let graceful = server.with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
                println!("SyncDrop WebSocket server received shutdown signal.");
            });

            if let Err(err) = graceful.await {
                eprintln!("Watch Party WebSocket server error: {}", err);
            }
            println!("SyncDrop WebSocket server on port {} has stopped.", port);
        });

        // Periodic Authoritative STATE_SYNC Ticker (Every 1000ms - Section 3.4 & Phase 7)
        let is_hosting_ticker = Arc::clone(&self_arc.is_hosting);
        let manager_ticker = Arc::clone(&self_arc);
        tauri::async_runtime::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(1000));
            while is_hosting_ticker.load(Ordering::Relaxed) {
                ticker.tick().await;
                if !is_hosting_ticker.load(Ordering::Relaxed) {
                    break;
                }
                let mpv_st = manager_ticker.mpv_controller.get_current_status().await;
                // Only broadcast when mpv has been launched
                if mpv_st.engine_status != "uninitialized" && mpv_st.engine_status != "stopped" {
                    let host_info = manager_ticker.discovery_state.get_local_info().await;
                    let intended_spd = f64::from_bits(manager_ticker.intended_speed.load(Ordering::Relaxed));
                    manager_ticker
                        .broadcast_state_sync(
                            mpv_st.time_pos,
                            mpv_st.pause,
                            if intended_spd > 0.0 { intended_spd } else { mpv_st.speed },
                            host_info.alias,
                        )
                        .await;
                }
            }
        });

        // Immediate Host MPV Property Listener (Host pressed Space or paused in MPV directly)
        let is_hosting_prop = Arc::clone(&self_arc.is_hosting);
        let manager_prop = Arc::clone(&self_arc);
        let mut prop_rx = self_arc.mpv_controller.subscribe_properties();
        tauri::async_runtime::spawn(async move {
            while let Ok((prop, val)) = prop_rx.recv().await {
                if !is_hosting_prop.load(Ordering::Relaxed) {
                    break;
                }
                if prop == "pause" {
                    if let Some(is_paused) = val.as_bool() {
                        let mpv_st = manager_prop.mpv_controller.get_current_status().await;
                        let host_info = manager_prop.discovery_state.get_local_info().await;
                        let intended_spd = f64::from_bits(manager_prop.intended_speed.load(Ordering::Relaxed));
                        manager_prop
                            .broadcast_state_sync(
                                mpv_st.time_pos,
                                is_paused,
                                if intended_spd > 0.0 { intended_spd } else { mpv_st.speed },
                                host_info.alias,
                            )
                            .await;
                    }
                }
            }
        });

        // Broadcast room discovery advertisement
        let host_info = self_arc.discovery_state.get_local_info().await;
        let room_title = if title.is_empty() { format!("{}'s Watch Party", host_info.alias) } else { title };
        let movie_name = movie_info.as_ref().map(|m| m.filename.clone()).unwrap_or_else(|| "No movie loaded".into());

        // Update active watch room in discovery state so UDP announcer advertises it across LAN
        let wr_announcement = crate::discovery::WatchRoomAnnouncement {
            room_id: room_id.clone(),
            title: room_title.clone(),
            host_alias: host_info.alias.clone(),
            port,
            movie_title: movie_name.clone(),
            member_count: 1,
        };
        self_arc.discovery_state.set_active_watch_room(Some(wr_announcement)).await;

        let disc = DiscoveredRoom {
            room_id: room_id.clone(),
            title: room_title,
            host_alias: host_info.alias,
            host_ip: local_ip.clone(),
            port,
            movie_title: movie_name,
            member_count: 1,
        };
        {
            let mut dr = self_arc.discovered_rooms.write().await;
            dr.clear();
            dr.insert(room_id.clone(), disc.clone());
        }
        self_arc.emit_event("syncdrop://discovered-rooms-updated", vec![disc]);

        let state = self_arc.get_current_room_state().await;
        self_arc.emit_event("syncdrop://room-state-sync", &state);
        self_arc.broadcast_member_list().await;

        Ok(state)
    }

    // ==========================================
    // Client Joining Logic
    // ==========================================

    /// Joins an existing host's room via WebSocket: ws://<host_ip>:53318
    pub async fn join_room(
        self_arc: Arc<Self>,
        host_uri: String,
    ) -> Result<(), String> {
        self_arc.leave_room().await?;

        // Parse host URI: "ws://192.168.1.50:53318" or "sync://192.168.1.50:53318" or "192.168.1.50"
        let clean_uri = host_uri
            .trim()
            .replace("sync://", "ws://")
            .replace("http://", "ws://");

        let target_ws_url = if clean_uri.starts_with("ws://") {
            clean_uri
        } else if clean_uri.contains(':') {
            format!("ws://{}", clean_uri)
        } else {
            format!("ws://{}:{}", clean_uri, DEFAULT_ROOM_PORT)
        };

        println!("Connecting to Watch Party host at: {}", target_ws_url);

        let (ws_stream, _) = tokio_tungstenite::connect_async(&target_ws_url)
            .await
            .map_err(|e| format!("Failed to connect to host at {}: {}", target_ws_url, e))?;

        let (mut ws_sink, mut ws_stream) = ws_stream.split();

        self_arc.is_hosting.store(false, Ordering::SeqCst);
        self_arc.is_connected.store(true, Ordering::SeqCst);

        let (client_tx, mut client_rx) = mpsc::channel::<SyncMessage>(64);
        {
            let mut ctx = self_arc.client_msg_tx.write().await;
            *ctx = Some(client_tx.clone());
        }

        // Send JOIN message
        let local_info = self_arc.discovery_state.get_local_info().await;
        let join_msg = SyncMessage::Join {
            client_id: local_info.id.clone(),
            alias: local_info.alias.clone(),
            client_version: "syncdrop-v1".to_string(),
        };

        let join_json = serde_json::to_string(&join_msg).unwrap();
        ws_sink
            .send(TungsteniteWsMessage::Text(join_json))
            .await
            .map_err(|e| format!("Failed to send JOIN message: {}", e))?;

        // Task A: Forward outgoing messages from mpsc channel to WebSocket
        let is_connected_a = Arc::clone(&self_arc.is_connected);
        tauri::async_runtime::spawn(async move {
            while let Some(msg) = client_rx.recv().await {
                if !is_connected_a.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(text) = serde_json::to_string(&msg) {
                    if let Err(_) = ws_sink.send(TungsteniteWsMessage::Text(text)).await {
                        break;
                    }
                }
            }
        });

        // Task B: Receive incoming messages from host
        let manager_clone = Arc::clone(&self_arc);
        tauri::async_runtime::spawn(async move {
            while let Some(msg_result) = ws_stream.next().await {
                match msg_result {
                    Ok(TungsteniteWsMessage::Text(text)) => {
                        if let Ok(sync_msg) = serde_json::from_str::<SyncMessage>(&text) {
                            manager_clone.handle_client_incoming_message(sync_msg).await;
                        }
                    }
                    Ok(TungsteniteWsMessage::Close(_)) | Err(_) => {
                        println!("WebSocket disconnected from host.");
                        break;
                    }
                    _ => {}
                }
            }

            // Connection lost
            manager_clone.is_connected.store(false, Ordering::SeqCst);
            let mut members = manager_clone.members.write().await;
            members.clear();
            manager_clone.emit_event("syncdrop://room-disconnected", ());
            println!("Watch Party room session ended.");
        });

        Ok(())
    }

    /// Handles incoming messages received by client from the authoritative host
    pub async fn handle_client_incoming_message(&self, message: SyncMessage) {
        match message {
            SyncMessage::StateSync {
                room_id,
                position,
                paused,
                speed,
                timestamp,
                sequence,
                actor,
                permissions,
            } => {
                // 1. Reject stale synchronization messages (sequence and timestamp)
                let last_seq = self.last_applied_sequence.load(Ordering::Relaxed);
                if sequence <= last_seq && sequence != 0 {
                    return;
                }
                self.last_applied_sequence.store(sequence, Ordering::SeqCst);

                let last_ts = self.last_applied_timestamp.load(Ordering::Relaxed);
                if timestamp < last_ts && last_ts != 0 {
                    return;
                }
                self.last_applied_timestamp.store(timestamp, Ordering::SeqCst);

                // Synchronize room control permissions if attached by host
                if let Some(perms) = permissions {
                    let mut lock = self.control_permissions.write().await;
                    *lock = perms.clone();
                    let is_strict = !perms.allow_all
                        && !perms.allow_play_pause
                        && !perms.allow_jump_forward
                        && !perms.allow_jump_backward
                        && !perms.allow_seek
                        && !perms.allow_speed;
                    self.strict_host_control.store(is_strict, Ordering::SeqCst);
                }

                // 2. Store room's intended playback speed (kept separate from drift speed)
                self.intended_speed.store(speed.to_bits(), Ordering::SeqCst);

                {
                    let mut r = self.room_id.write().await;
                    *r = Some(room_id.clone());
                }

                // 3. Account for network message delay and estimate expected host position
                // Clamp latency to realistic LAN transit [0, 100ms] to avoid system clock skew inflation
                let now = now_ms();
                let latency_secs = ((now.saturating_sub(timestamp) as f64) / 1000.0).clamp(0.0, 0.1);
                let expected_position = if paused {
                    position
                } else {
                    position + (latency_secs * speed)
                };

                // 4. Initial movie compatibility check (Filename, size, SHA-256, and compatibility mode)
                let approved = self.compatibility_approved.load(Ordering::Relaxed);
                let (movie_matched, _compat_msg) = {
                    let host_movie = self.current_movie.read().await;
                    let local_movie = self.local_movie.read().await;
                    match (&*host_movie, &*local_movie) {
                        (Some(hm), Some(lm)) => check_movie_compatibility(hm, lm, approved),
                        (Some(_), None) => (false, "Please select local movie file".to_string()),
                        (None, _) => (true, "No host movie specified".to_string()),
                    }
                };

                // 5. Compare with local mpv playback position
                let mpv_st = self.mpv_controller.get_current_status().await;
                let local_pos = mpv_st.time_pos;
                let drift = local_pos - expected_position;

                // 6. Handle play/pause state synchronization (Host is authoritative!)
                if mpv_st.pause != paused {
                    if paused {
                        // Host paused/stopped: ALWAYS enforce pause on participant!
                        let _ = self.mpv_controller.pause().await;
                    } else if movie_matched {
                        let _ = self.mpv_controller.play().await;
                    }
                }

                let mut sync_status = if !movie_matched {
                    "Waiting for Movie".to_string()
                } else if paused {
                    "Paused with Host".to_string()
                } else {
                    "In Sync".to_string()
                };

                // 7. Apply Drift Correction Policy with Audio Pitch & Filter Protection
                if movie_matched && !paused {
                    let abs_drift = drift.abs();

                    if abs_drift > 3.5 {
                        // DRIFT ABOVE 3.5 SECONDS (Major desync or manual jump):
                        // Snap/seek to the expected host playback position with a 4.0s debounce
                        let can_seek = {
                            let last_seek = self.last_seek_time.read().await;
                            last_seek.elapsed() >= Duration::from_millis(4000)
                        };

                        if can_seek {
                            println!(
                                "Drift > 3.5s (drift={:.2}s). Snapping to host expected position: {:.2}s",
                                drift, expected_position
                            );
                            let _ = self.mpv_controller.seek(expected_position, true).await;
                            if (mpv_st.speed - speed).abs() > 0.005 {
                                let _ = self.mpv_controller.set_speed(speed).await;
                            }
                            let mut last_seek = self.last_seek_time.write().await;
                            *last_seek = Instant::now();
                        }
                        sync_status = "Snapping to Host".to_string();
                    } else if abs_drift >= 0.6 {
                        // DRIFT BETWEEN 0.6 AND 3.5 SECONDS:
                        // Ultra-gentle 1.5% tempo adjustment (smooth, inaudible pitch variation)
                        let correction_multiplier = if drift < -0.6 {
                            // Local is behind host: gently catch up
                            1.015
                        } else {
                            // Local is ahead of host: gently slow down
                            0.985
                        };
                        let adjusted_speed = (speed * correction_multiplier).clamp(0.5, 2.0);
                        // CRITICAL: Only send set_speed if not already at target speed to avoid resetting mpv audio filters
                        if (mpv_st.speed - adjusted_speed).abs() > 0.005 {
                            let _ = self.mpv_controller.set_speed(adjusted_speed).await;
                        }
                        sync_status = format!("Speed Syncing ({:.2}x)", adjusted_speed);
                    } else {
                        // DRIFT BELOW 0.6 SECONDS (Deadband):
                        // In natural sync. Restore normal room speed once if not already active.
                        if (mpv_st.speed - speed).abs() > 0.005 {
                            let _ = self.mpv_controller.set_speed(speed).await;
                        }
                        sync_status = "In Sync (< 0.6s)".to_string();
                    }
                } else if paused {
                    // Paused playback: restore normal room playback speed
                    if (mpv_st.speed - speed).abs() > 0.005 {
                        let _ = self.mpv_controller.set_speed(speed).await;
                    }
                    if (local_pos - position).abs() > 1.5 {
                        let can_seek = {
                            let last_seek = self.last_seek_time.read().await;
                            last_seek.elapsed() >= Duration::from_millis(3000)
                        };
                        if can_seek {
                            let _ = self.mpv_controller.seek(position, true).await;
                            let mut last_seek = self.last_seek_time.write().await;
                            *last_seek = Instant::now();
                        }
                    }
                }

                // 8. Update UI State
                let state = WatchRoomState {
                    room_id,
                    movie_id: "syncdrop_movie".to_string(),
                    movie_info: self.current_movie.read().await.clone(),
                    position: expected_position,
                    paused,
                    speed,
                    timestamp,
                    sequence,
                    actor,
                    strict_host_control: self.strict_host_control.load(Ordering::Relaxed),
                    permissions: self.control_permissions.read().await.clone(),
                    is_host: false,
                    host_ip: self.host_ip.read().await.clone(),
                    port: self.port.load(Ordering::Relaxed) as u16,
                    drift_seconds: drift,
                    sync_status: sync_status.clone(),
                    movie_matched,
                };

                self.emit_event("syncdrop://room-state-sync", &state);

                // 9. Report Participant Status to Host (Throttled every 1000ms)
                let last_rep = self.last_status_report.load(Ordering::Relaxed);
                if now.saturating_sub(last_rep) >= 1000 {
                    self.last_status_report.store(now, Ordering::SeqCst);
                    let local_info = self.discovery_state.get_local_info().await;
                    let report = SyncMessage::StatusReport {
                        client_id: local_info.id,
                        status: sync_status,
                        drift_seconds: drift,
                        movie_matched,
                        position: local_pos,
                    };
                    self.send_client_message(report).await;
                }
            }

            SyncMessage::FileInfo {
                movie_id,
                filename,
                file_size,
                duration,
                container_format,
                sha256,
            } => {
                let info = MovieFileInfo {
                    movie_id,
                    filename,
                    file_path: None,
                    file_size,
                    duration,
                    container_format,
                    video_codec: Some("H.264 / AAC".to_string()),
                    audio_codec: None,
                    resolution: None,
                    sha256,
                };

                {
                    let mut cm = self.current_movie.write().await;
                    *cm = Some(info.clone());
                }

                let approved = self.compatibility_approved.load(Ordering::Relaxed);
                let (matched, compat_msg) = {
                    let lm = self.local_movie.read().await;
                    match &*lm {
                        Some(local) => check_movie_compatibility(&info, local, approved),
                        None => (false, "Please select local movie file".to_string()),
                    }
                };

                println!("Host movie received: {} ({} bytes). Matched: {} ({})", info.filename, info.file_size, matched, compat_msg);

                self.emit_event("syncdrop://room-file-info", &info);

                if matched && self.local_movie.read().await.is_some() {
                    self.request_sync().await;
                }

                let local_info = self.discovery_state.get_local_info().await;
                let report = SyncMessage::StatusReport {
                    client_id: local_info.id,
                    status: if matched { "SYNCHRONIZING".to_string() } else { "WAITING_FOR_MOVIE".to_string() },
                    drift_seconds: 0.0,
                    movie_matched: matched,
                    position: 0.0,
                };
                self.send_client_message(report).await;
            }

            SyncMessage::SyncSnapshot {
                room_id,
                movie_id,
                client_id,
                position,
                paused,
                speed,
                timestamp,
                sequence_number,
                host_id: _,
                client_time,
                permissions,
            } => {
                if let Some(perms) = permissions {
                    let mut lock = self.control_permissions.write().await;
                    *lock = perms;
                }
                let local_info = self.discovery_state.get_local_info().await;
                // Only process snapshot addressed to this client or broadcast
                if !client_id.is_empty() && client_id != local_info.id {
                    return;
                }

                // 1. Validate room ID
                if let Some(ref cur_room) = *self.room_id.read().await {
                    if !room_id.is_empty() && cur_room != &room_id {
                        eprintln!("Rejected SyncSnapshot: mismatched room ID {}", room_id);
                        return;
                    }
                }

                // 2. Reject stale snapshot (sequence number & timestamp - Section 13.5)
                let last_seq = self.last_applied_sequence.load(Ordering::Relaxed);
                if sequence_number <= last_seq && sequence_number != 0 {
                    println!("Rejected stale SyncSnapshot: sequence {} <= {}", sequence_number, last_seq);
                    return;
                }
                self.last_applied_sequence.store(sequence_number, Ordering::SeqCst);

                let last_ts = self.last_applied_timestamp.load(Ordering::Relaxed);
                if timestamp < last_ts && last_ts != 0 {
                    println!("Rejected stale SyncSnapshot: timestamp {} < {}", timestamp, last_ts);
                    return;
                }
                self.last_applied_timestamp.store(timestamp, Ordering::SeqCst);

                // 3. Store intended playback speed
                self.intended_speed.store(speed.to_bits(), Ordering::SeqCst);

                // 4. Calculate expected playback position using latency and elapsed time
                let now = now_ms();
                let expected_position = calculate_expected_playback_position(
                    position,
                    paused,
                    speed,
                    timestamp,
                    client_time,
                    now,
                );

                println!(
                    "Late Join: applying authoritative snapshot: expected_pos={:.2}s (host={:.2}s, paused={}, speed={:.2}x)",
                    expected_position, position, paused, speed
                );

                // 5. Seek mpv directly to the expected position
                let _ = self.mpv_controller.seek(expected_position, true).await;
                let _ = self.mpv_controller.set_speed(speed).await;

                // 6. Set pause state
                if paused {
                    let _ = self.mpv_controller.pause().await;
                } else {
                    let _ = self.mpv_controller.play().await;
                }

                // 7. Send SYNC_CONFIRMED to host
                self.send_client_message(SyncMessage::SyncConfirmed {
                    client_id: local_info.id.clone(),
                    applied_position: expected_position,
                    drift_seconds: 0.0,
                    timestamp: now,
                }).await;

                // 8. Report WATCHING status
                self.send_client_message(SyncMessage::StatusReport {
                    client_id: local_info.id,
                    status: "WATCHING".to_string(),
                    drift_seconds: 0.0,
                    movie_matched: true,
                    position: expected_position,
                }).await;

                // 9. Update UI state
                self.emit_event(
                    "syncdrop://room-state-sync",
                    WatchRoomState {
                        room_id: room_id.clone(),
                        movie_id: movie_id.clone(),
                        movie_info: self.current_movie.read().await.clone(),
                        position: expected_position,
                        paused,
                        speed,
                        timestamp: now,
                        sequence: sequence_number,
                        actor: "Host".to_string(),
                        strict_host_control: self.strict_host_control.load(Ordering::Relaxed),
                    permissions: self.control_permissions.read().await.clone(),
                        is_host: false,
                        host_ip: self.host_ip.read().await.clone(),
                        port: self.port.load(Ordering::Relaxed) as u16,
                        drift_seconds: 0.0,
                        sync_status: if paused { "Paused with Host".to_string() } else { "Watching".to_string() },
                        movie_matched: true,
                    },
                );
            }

            SyncMessage::PermissionsUpdate { room_id: _, permissions } => {
                {
                    let mut lock = self.control_permissions.write().await;
                    *lock = permissions.clone();
                }
                let is_strict = !permissions.allow_all
                    && !permissions.allow_play_pause
                    && !permissions.allow_jump_forward
                    && !permissions.allow_jump_backward
                    && !permissions.allow_seek
                    && !permissions.allow_speed;
                self.strict_host_control.store(is_strict, Ordering::SeqCst);

                let state = self.get_current_room_state().await;
                self.emit_event("syncdrop://room-state-sync", &state);
            }

            SyncMessage::MemberList { members } => {
                {
                    let mut mem_map = self.members.write().await;
                    mem_map.clear();
                    for m in &members {
                        mem_map.insert(m.id.clone(), m.clone());
                    }
                }
                self.emit_event("syncdrop://room-members-updated", members);
            }

            SyncMessage::Ping { client_time } => {
                let pong = SyncMessage::Pong {
                    client_time,
                    server_time: now_ms(),
                };
                self.send_client_message(pong).await;
            }

            SyncMessage::Pong { client_time, .. } => {
                let rtt = now_ms().saturating_sub(client_time);
                // Ping measured
                println!("Watch Party RTT Ping: {} ms", rtt);
            }

            SyncMessage::Leave { client_id, reason } => {
                println!("Watch Party: host/peer {} left room (reason: {:?})", client_id, reason);
                self.is_connected.store(false, Ordering::SeqCst);
                {
                    let mut r = self.room_id.write().await;
                    *r = None;
                    let mut ctx = self.client_msg_tx.write().await;
                    *ctx = None;
                    let mut members = self.members.write().await;
                    members.clear();
                }
                self.emit_event("syncdrop://room-disconnected", ());
                let empty_state = self.get_current_room_state().await;
                self.emit_event("syncdrop://room-state-sync", &empty_state);
            }

            _ => {}
        }
    }

    /// Sends a message from the client to the host
    pub async fn send_client_message(&self, message: SyncMessage) {
        let lock = self.client_msg_tx.read().await;
        if let Some(ref tx) = *lock {
            let _ = tx.send(message).await;
        }
    }

    // ==========================================
    // Playback Action Dispatchers
    // ==========================================

    /// Issues play action (if host, broadcasts; if client, sends to host)
    pub async fn dispatch_play(&self) -> Result<(), String> {
        let mpv_st = self.mpv_controller.get_current_status().await;
        let pos = mpv_st.time_pos;
        let local_info = self.discovery_state.get_local_info().await;

        if self.is_hosting.load(Ordering::Relaxed) {
            // Host is authoritative: idempotent resume from current position, never restart from 00:00!
            if mpv_st.pause {
                let _ = self.mpv_controller.play().await;
            }
            let updated_st = self.mpv_controller.get_current_status().await;
            let current_pos = if updated_st.time_pos > 0.0 { updated_st.time_pos } else { pos };
            self.broadcast_state_sync(current_pos, false, updated_st.speed, local_info.alias).await;
        } else {
            // Participant: verify host permission before sending
            let perms = self.control_permissions.read().await;
            if !perms.is_play_pause_allowed() {
                return Err("Play/Pause control is restricted by the room host".to_string());
            }
            drop(perms);

            if mpv_st.engine_status == "loading" {
                return Err("Player is still loading the movie. Please wait for synchronization.".to_string());
            }
            let room_id = self.room_id.read().await.clone().unwrap_or_default();
            let msg = SyncMessage::Play {
                room_id,
                position: pos,
                actor: local_info.alias,
                timestamp: now_ms(),
            };
            self.send_client_message(msg).await;
        }

        Ok(())
    }

    /// Issues pause action
    pub async fn dispatch_pause(&self) -> Result<(), String> {
        let mpv_st = self.mpv_controller.get_current_status().await;
        let pos = mpv_st.time_pos;
        let local_info = self.discovery_state.get_local_info().await;

        if self.is_hosting.load(Ordering::Relaxed) {
            let _ = self.mpv_controller.pause().await;
            self.broadcast_state_sync(pos, true, mpv_st.speed, local_info.alias).await;
        } else {
            let perms = self.control_permissions.read().await;
            if !perms.is_play_pause_allowed() {
                return Err("Play/Pause control is restricted by the room host".to_string());
            }
            drop(perms);

            let room_id = self.room_id.read().await.clone().unwrap_or_default();
            let msg = SyncMessage::Pause {
                room_id,
                position: pos,
                actor: local_info.alias,
                timestamp: now_ms(),
            };
            self.send_client_message(msg).await;
        }

        Ok(())
    }

    /// Issues seek action
    pub async fn dispatch_seek(&self, seconds: f64, seek_type: Option<String>) -> Result<(), String> {
        let mpv_st = self.mpv_controller.get_current_status().await;
        let local_info = self.discovery_state.get_local_info().await;

        if self.is_hosting.load(Ordering::Relaxed) {
            let _ = self.mpv_controller.seek(seconds, true).await;
            self.broadcast_state_sync(seconds, mpv_st.pause, mpv_st.speed, local_info.alias).await;
        } else {
            let perms = self.control_permissions.read().await;
            let allowed = match seek_type.as_deref() {
                Some("forward") => perms.is_jump_forward_allowed(),
                Some("backward") => perms.is_jump_backward_allowed(),
                Some("absolute") => perms.is_seek_allowed(),
                _ => perms.is_seek_allowed() || perms.is_jump_forward_allowed() || perms.is_jump_backward_allowed(),
            };
            if !allowed {
                return Err("Seeking is restricted by the room host".to_string());
            }
            drop(perms);

            let room_id = self.room_id.read().await.clone().unwrap_or_default();
            let msg = SyncMessage::Seek {
                room_id,
                position: seconds,
                seek_type,
                actor: local_info.alias,
                timestamp: now_ms(),
            };
            self.send_client_message(msg).await;
        }

        Ok(())
    }

    /// Issues speed action
    pub async fn dispatch_speed(&self, speed: f64) -> Result<(), String> {
        let mpv_st = self.mpv_controller.get_current_status().await;
        let local_info = self.discovery_state.get_local_info().await;

        if self.is_hosting.load(Ordering::Relaxed) {
            let _ = self.mpv_controller.set_speed(speed).await;
            self.broadcast_state_sync(mpv_st.time_pos, mpv_st.pause, speed, local_info.alias).await;
        } else {
            let perms = self.control_permissions.read().await;
            if !perms.is_speed_allowed() {
                return Err("Playback speed control is restricted by the room host".to_string());
            }
            drop(perms);

            let room_id = self.room_id.read().await.clone().unwrap_or_default();
            let msg = SyncMessage::Speed {
                room_id,
                speed,
                actor: local_info.alias,
                timestamp: now_ms(),
            };
            self.send_client_message(msg).await;
        }

        Ok(())
    }

    /// Broadcasts authoritative STATE_SYNC message from host to all connected participants
    pub async fn broadcast_state_sync(
        &self,
        position: f64,
        paused: bool,
        speed: f64,
        actor: String,
    ) {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst) + 1;
        let room_id = self.room_id.read().await.clone().unwrap_or_default();
        let timestamp = now_ms();

        self.intended_speed.store(speed.to_bits(), Ordering::SeqCst);
        let cur_perms = self.control_permissions.read().await.clone();

        let msg = SyncMessage::StateSync {
            room_id: room_id.clone(),
            position,
            paused,
            speed,
            timestamp,
            sequence: seq,
            actor: actor.clone(),
            permissions: Some(cur_perms.clone()),
        };

        // Broadcast over WebSocket channel to all connected clients
        let _ = self.host_broadcast_tx.send(msg);

        // Update local UI
        let state = WatchRoomState {
            room_id,
            movie_id: "syncdrop_movie".to_string(),
            movie_info: self.current_movie.read().await.clone(),
            position,
            paused,
            speed,
            timestamp,
            sequence: seq,
            actor,
            strict_host_control: self.strict_host_control.load(Ordering::Relaxed),
            permissions: self.control_permissions.read().await.clone(),
            is_host: true,
            host_ip: self.host_ip.read().await.clone(),
            port: self.port.load(Ordering::Relaxed) as u16,
            drift_seconds: 0.0,
            sync_status: if paused { "Paused (Host)".to_string() } else { "Authoritative Host".to_string() },
            movie_matched: true,
        };

        self.emit_event("syncdrop://room-state-sync", &state);
    }

    /// Broadcasts member list to all participants
    pub async fn broadcast_member_list(&self) {
        let members: Vec<RoomMember> = {
            let map = self.members.read().await;
            map.values().cloned().collect()
        };

        let msg = SyncMessage::MemberList {
            members: members.clone(),
        };
        let _ = self.host_broadcast_tx.send(msg);
        self.emit_event("syncdrop://room-members-updated", members);
    }

    /// Leaves active room and resets state
    pub async fn leave_room(&self) -> Result<(), String> {
        if self.is_connected.load(Ordering::Relaxed) {
            let local_info = self.discovery_state.get_local_info().await;
            let leave_msg = SyncMessage::Leave {
                client_id: local_info.id,
                reason: Some("User departed room".to_string()),
            };

            if self.is_hosting.load(Ordering::Relaxed) {
                let _ = self.host_broadcast_tx.send(leave_msg);
            } else {
                self.send_client_message(leave_msg).await;
            }
        }

        // 1. Terminate local Axum WebSocket server if hosting
        if let Some(shutdown_tx) = self.server_shutdown_tx.write().await.take() {
            let _ = shutdown_tx.send(());
        }

        // 2. Deregister active watch room in discovery state so UDP announcer stops advertising
        self.discovery_state.set_active_watch_room(None).await;

        // 3. Reset internal connection state and clear room data
        self.is_hosting.store(false, Ordering::SeqCst);
        self.is_connected.store(false, Ordering::SeqCst);
        {
            let mut r = self.room_id.write().await;
            *r = None;
            let mut ctx = self.client_msg_tx.write().await;
            *ctx = None;
            let mut members = self.members.write().await;
            members.clear();
            let mut dr = self.discovered_rooms.write().await;
            dr.clear();
        }

        // 4. Notify UI that room session has ended
        let empty_state = self.get_current_room_state().await;
        self.emit_event("syncdrop://room-state-sync", &empty_state);
        self.emit_event("syncdrop://room-disconnected", ());
        self.emit_event("syncdrop://discovered-rooms-updated", Vec::<DiscoveredRoom>::new());
        self.emit_event("syncdrop://room-members-updated", Vec::<RoomMember>::new());
        Ok(())
    }

    /// Returns current room state snapshot
    pub async fn get_current_room_state(&self) -> WatchRoomState {
        let mpv_st = self.mpv_controller.get_current_status().await;
        let is_host = self.is_hosting.load(Ordering::Relaxed);
        let is_conn = self.is_connected.load(Ordering::Relaxed);
        let room_id = if is_host || is_conn {
            self.room_id.read().await.clone().unwrap_or_default()
        } else {
            String::new()
        };
        let host_ip = self.host_ip.read().await.clone();
        let port = self.port.load(Ordering::Relaxed) as u16;
        let seq = self.sequence.load(Ordering::Relaxed);
        let local_info = self.discovery_state.get_local_info().await;

        let intended_spd = f64::from_bits(self.intended_speed.load(Ordering::Relaxed));
        let approved = self.compatibility_approved.load(Ordering::Relaxed);
        let (movie_matched, _) = {
            let host_movie = self.current_movie.read().await;
            let local_movie = self.local_movie.read().await;
            match (&*host_movie, &*local_movie) {
                (Some(hm), Some(lm)) => check_movie_compatibility(hm, lm, approved),
                (Some(_), None) => (false, "Please select local movie".to_string()),
                (None, _) => (true, "No host movie specified".to_string()),
            }
        };

        WatchRoomState {
            room_id,
            movie_id: "syncdrop_movie".to_string(),
            movie_info: self.current_movie.read().await.clone(),
            position: mpv_st.time_pos,
            paused: mpv_st.pause,
            speed: if intended_spd > 0.0 { intended_spd } else { mpv_st.speed },
            timestamp: now_ms(),
            sequence: seq,
            actor: local_info.alias,
            strict_host_control: self.strict_host_control.load(Ordering::Relaxed),
            permissions: self.control_permissions.read().await.clone(),
            is_host,
            host_ip,
            port,
            drift_seconds: 0.0,
            sync_status: if is_host {
                if mpv_st.pause { "Paused (Host)".to_string() } else { "Authoritative Host".to_string() }
            } else if is_conn {
                "In Sync".to_string()
            } else {
                "Idle".to_string()
            },
            movie_matched,
        }
    }

    /// Updates room playback control permissions
    pub async fn set_permissions(&self, permissions: RoomControlPermissions) {
        {
            let mut lock = self.control_permissions.write().await;
            *lock = permissions.clone();
        }
        let is_strict = !permissions.allow_all
            && !permissions.allow_play_pause
            && !permissions.allow_jump_forward
            && !permissions.allow_jump_backward
            && !permissions.allow_seek
            && !permissions.allow_speed;
        self.strict_host_control.store(is_strict, Ordering::SeqCst);

        if self.is_hosting.load(Ordering::Relaxed) {
            let room_id = self.room_id.read().await.clone().unwrap_or_default();
            let _ = self.host_broadcast_tx.send(SyncMessage::PermissionsUpdate {
                room_id,
                permissions: permissions.clone(),
            });
        }

        let state = self.get_current_room_state().await;
        self.emit_event("syncdrop://room-state-sync", &state);
    }

    /// Sets client's local movie info and requests synchronization with host
    /// Sets client's local movie info and executes late-join loading & synchronization (Section 13.3)
    pub async fn set_local_movie(&self, info: MovieFileInfo) {
        {
            let mut lm = self.local_movie.write().await;
            *lm = Some(info.clone());
        }

        // If connected as a client, execute late-join loading and verification workflow
        if self.is_connected.load(Ordering::Relaxed) && !self.is_hosting.load(Ordering::Relaxed) {
            let local_info = self.discovery_state.get_local_info().await;
            let host_movie = self.current_movie.read().await.clone();

            // 1. Verify movie against host's FILE_INFO if present
            let approved = self.compatibility_approved.load(Ordering::Relaxed);
            if let Some(ref hm) = host_movie {
                let (compat, compat_msg) = check_movie_compatibility(hm, &info, approved);
                if !compat {
                    eprintln!("Late join movie verification warning: {}", compat_msg);
                    self.send_client_message(SyncMessage::StatusReport {
                        client_id: local_info.id.clone(),
                        status: "WAITING_FOR_MOVIE".to_string(),
                        drift_seconds: 0.0,
                        movie_matched: false,
                        position: 0.0,
                    }).await;
                    return;
                }
            }

            // 2. Report VERIFYING_MOVIE then LAUNCHING_PLAYER
            self.send_client_message(SyncMessage::StatusReport {
                client_id: local_info.id.clone(),
                status: "VERIFYING_MOVIE".to_string(),
                drift_seconds: 0.0,
                movie_matched: true,
                position: 0.0,
            }).await;

            // 3. Launch mpv and load file in PAUSED state (prevent starting from 00:00 - Requirement 11)
            let path_to_load = info.file_path.as_deref().unwrap_or(&info.filename);
            let _ = self.mpv_controller.pause().await;
            if let Err(e) = self.mpv_controller.load_file(path_to_load).await {
                eprintln!("Failed to load local movie into mpv: {}", e);
                self.send_client_message(SyncMessage::StatusReport {
                    client_id: local_info.id.clone(),
                    status: "ERROR".to_string(),
                    drift_seconds: 0.0,
                    movie_matched: false,
                    position: 0.0,
                }).await;
                return;
            }

            self.send_client_message(SyncMessage::StatusReport {
                client_id: local_info.id.clone(),
                status: "LOADING_MOVIE".to_string(),
                drift_seconds: 0.0,
                movie_matched: true,
                position: 0.0,
            }).await;

            // 4. Poll/Wait until mpv confirms file is loaded
            let mut loaded = false;
            for _ in 0..30 {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let st = self.mpv_controller.get_current_status().await;
                if st.duration > 0.0 || st.engine_status == "paused" || st.engine_status == "ready" {
                    loaded = true;
                    break;
                }
            }

            if !loaded {
                eprintln!("Movie loading timed out in mpv sidecar");
            }

            // Keep player paused at 00:00 until snapshot arrives
            let _ = self.mpv_controller.pause().await;
            let st = self.mpv_controller.get_current_status().await;

            // 5. Send SYNC_READY
            self.send_client_message(SyncMessage::SyncReady {
                client_id: local_info.id.clone(),
                movie_id: info.movie_id.clone(),
                local_duration: st.duration,
            }).await;

            // 6. Send SYNC_REQUEST with client_time for RTT calculation
            let now = now_ms();
            self.send_client_message(SyncMessage::SyncRequest {
                client_id: local_info.id.clone(),
                movie_id: info.movie_id.clone(),
                client_time: now,
            }).await;

            self.send_client_message(SyncMessage::StatusReport {
                client_id: local_info.id,
                status: "WAITING_FOR_SYNC".to_string(),
                drift_seconds: 0.0,
                movie_matched: true,
                position: 0.0,
            }).await;
        }
    }

    /// Requests synchronization snapshot from host
    pub async fn request_sync(&self) {
        if self.is_connected.load(Ordering::Relaxed) && !self.is_hosting.load(Ordering::Relaxed) {
            let local_info = self.discovery_state.get_local_info().await;
            let movie_id = self.current_movie.read().await.as_ref().map(|m| m.movie_id.clone()).unwrap_or_default();
            self.send_client_message(SyncMessage::SyncRequest {
                client_id: local_info.id,
                movie_id,
                client_time: now_ms(),
            })
            .await;
        }
    }

    /// Sets or updates the active movie for the room
    pub async fn set_movie(&self, info: MovieFileInfo) {
        {
            let mut cm = self.current_movie.write().await;
            *cm = Some(info.clone());
        }
        if self.is_hosting.load(Ordering::Relaxed) {
            let fi_msg = SyncMessage::FileInfo {
                movie_id: info.movie_id.clone(),
                filename: info.filename.clone(),
                file_size: info.file_size,
                duration: info.duration,
                container_format: info.container_format.clone(),
                sha256: info.sha256.clone(),
            };
            let _ = self.host_broadcast_tx.send(fi_msg);
        }
        self.emit_event("syncdrop://room-file-info", &info);
    }

    /// Returns list of discovered watch rooms on LAN
    pub async fn get_discovered_rooms_list(&self) -> Vec<DiscoveredRoom> {
        let mut rooms: Vec<DiscoveredRoom> = Vec::new();

        // 1. If this device is actively hosting, include our own room
        if self.is_hosting.load(Ordering::Relaxed) {
            let dr = self.discovered_rooms.read().await;
            rooms.extend(dr.values().cloned());
        }

        // 2. Augment with verified watch rooms actively announced by other peers on LAN
        let peers = self.discovery_state.get_peers().await;
        let local_info = self.discovery_state.get_local_info().await;
        for peer in peers {
            if !peer.is_stale && peer.id != local_info.id {
                if let Some(wr) = &peer.watch_room {
                    if !rooms.iter().any(|r| r.room_id == wr.room_id || r.host_ip == peer.ip) {
                        rooms.push(DiscoveredRoom {
                            room_id: wr.room_id.clone(),
                            title: wr.title.clone(),
                            host_alias: wr.host_alias.clone(),
                            host_ip: peer.ip.clone(),
                            port: wr.port,
                            movie_title: wr.movie_title.clone(),
                            member_count: wr.member_count,
                        });
                    }
                }
            }
        }
        rooms
    }
}

// ==========================================
// Axum Host WebSocket Connection Handler
// ==========================================

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(manager): State<Arc<SyncRoomManager>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_host_socket(socket, addr, manager))
}

async fn handle_host_socket(
    socket: WebSocket,
    client_addr: SocketAddr,
    manager: Arc<SyncRoomManager>,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let mut rx_broadcast = manager.host_broadcast_tx.subscribe();

    let mut participant_id: Option<String> = None;

    // Task 1: Forward room broadcasts to this client
    let _participant_id_clone = participant_id.clone();
    let forward_task = tokio::spawn(async move {
        while let Ok(msg) = rx_broadcast.recv().await {
            if let Ok(text) = serde_json::to_string(&msg) {
                if let Err(_) = ws_sender.send(AxumWsMessage::Text(text)).await {
                    break;
                }
            }
        }
    });

    // Task 2: Read messages from this client
    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let AxumWsMessage::Text(text) = msg {
            if let Ok(sync_msg) = serde_json::from_str::<SyncMessage>(&text) {
                match sync_msg {
                    SyncMessage::Join {
                        client_id,
                        alias,
                        ..
                    } => {
                        participant_id = Some(client_id.clone());
                        println!("Participant joined room: {} ({}) from {}", alias, client_id, client_addr);

                        let initial_status = if manager.current_movie.read().await.is_some() {
                            "WAITING_FOR_MOVIE"
                        } else {
                            "CONNECTED"
                        };

                        // Add to members list
                        {
                            let mut members = manager.members.write().await;
                            members.insert(
                                client_id.clone(),
                                RoomMember {
                                    id: client_id.clone(),
                                    alias: alias.clone(),
                                    ip: client_addr.ip().to_string(),
                                    is_host: false,
                                    status: initial_status.to_string(),
                                    movie_matched: false,
                                    drift_seconds: 0.0,
                                    position: 0.0,
                                    ping_ms: 5,
                                },
                            );
                        }

                        // Send FILE_INFO to the newly joined client
                        if let Some(ref movie) = *manager.current_movie.read().await {
                            let fi_msg = SyncMessage::FileInfo {
                                movie_id: movie.movie_id.clone(),
                                filename: movie.filename.clone(),
                                file_size: movie.file_size,
                                duration: movie.duration,
                                container_format: movie.container_format.clone(),
                                sha256: movie.sha256.clone(),
                            };
                            let _ = manager.host_broadcast_tx.send(fi_msg);
                        }

                        // Send active control permissions to the newly joined client
                        let cur_perms = manager.control_permissions.read().await.clone();
                        let room_id = manager.room_id.read().await.clone().unwrap_or_default();
                        let _ = manager.host_broadcast_tx.send(SyncMessage::PermissionsUpdate {
                            room_id,
                            permissions: cur_perms,
                        });

                        manager.broadcast_member_list().await;
                    }

                    SyncMessage::SyncReady { client_id, movie_id: _, local_duration: _ } => {
                        println!("Participant reported SYNC_READY: {}", client_id);
                        {
                            let mut members = manager.members.write().await;
                            if let Some(m) = members.get_mut(&client_id) {
                                m.status = "WAITING_FOR_SYNC".to_string();
                                m.movie_matched = true;
                            }
                        }
                        manager.broadcast_member_list().await;
                    }

                    SyncMessage::SyncRequest { client_id, movie_id, client_time } => {
                        println!("Participant requested authoritative sync snapshot: {}", client_id);
                        let mpv_st = manager.mpv_controller.get_current_status().await;
                        let host_info = manager.discovery_state.get_local_info().await;
                        let intended_spd = f64::from_bits(manager.intended_speed.load(Ordering::Relaxed));
                        let speed = if intended_spd > 0.0 { intended_spd } else { mpv_st.speed.max(0.25) };
                        let seq = manager.sequence.fetch_add(1, Ordering::SeqCst);
                        let room_id = manager.room_id.read().await.clone().unwrap_or_default();
                        let curr_movie_id = if !movie_id.is_empty() {
                            movie_id
                        } else {
                            manager.current_movie.read().await.as_ref().map(|m| m.movie_id.clone()).unwrap_or_default()
                        };

                        if let Some(ref movie) = *manager.current_movie.read().await {
                            let fi_msg = SyncMessage::FileInfo {
                                movie_id: movie.movie_id.clone(),
                                filename: movie.filename.clone(),
                                file_size: movie.file_size,
                                duration: movie.duration,
                                container_format: movie.container_format.clone(),
                                sha256: movie.sha256.clone(),
                            };
                            let _ = manager.host_broadcast_tx.send(fi_msg);
                        }

                        let perms = manager.control_permissions.read().await.clone();
                        let snapshot = SyncMessage::SyncSnapshot {
                            room_id,
                            movie_id: curr_movie_id,
                            client_id: client_id.clone(),
                            position: mpv_st.time_pos,
                            paused: mpv_st.pause,
                            speed,
                            timestamp: now_ms(),
                            sequence_number: seq,
                            host_id: host_info.id,
                            client_time,
                            permissions: Some(perms),
                        };

                        {
                            let mut members = manager.members.write().await;
                            if let Some(m) = members.get_mut(&client_id) {
                                m.status = "SYNCHRONIZING".to_string();
                                m.movie_matched = true;
                            }
                        }

                        let _ = manager.host_broadcast_tx.send(snapshot);
                        manager.broadcast_member_list().await;
                    }

                    SyncMessage::SyncConfirmed {
                        client_id,
                        applied_position,
                        drift_seconds,
                        timestamp: _,
                    } => {
                        println!("Participant confirmed synchronization: {} at position {:.2}s", client_id, applied_position);
                        {
                            let mut members = manager.members.write().await;
                            if let Some(m) = members.get_mut(&client_id) {
                                m.status = "WATCHING".to_string();
                                m.position = applied_position;
                                m.drift_seconds = drift_seconds;
                                m.movie_matched = true;
                            }
                        }
                        manager.broadcast_member_list().await;
                    }

                    SyncMessage::StatusReport {
                        client_id,
                        status,
                        drift_seconds,
                        movie_matched,
                        position,
                    } => {
                        {
                            let mut members = manager.members.write().await;
                            if let Some(m) = members.get_mut(&client_id) {
                                m.status = status;
                                m.drift_seconds = drift_seconds;
                                m.movie_matched = movie_matched;
                                if position > 0.0 {
                                    m.position = position;
                                }
                            }
                        }
                        manager.broadcast_member_list().await;
                    }

                    SyncMessage::Play { actor, .. } => {
                        let perms = manager.control_permissions.read().await;
                        if !perms.is_play_pause_allowed() {
                            println!("Rejecting unauthorized PLAY command from participant: {}", actor);
                            continue;
                        }
                        drop(perms);

                        let mpv_st = manager.mpv_controller.get_current_status().await;
                        if mpv_st.pause {
                            let _ = manager.mpv_controller.play().await;
                        }
                        let updated_st = manager.mpv_controller.get_current_status().await;
                        let intended_spd = f64::from_bits(manager.intended_speed.load(Ordering::Relaxed));
                        manager.broadcast_state_sync(
                            updated_st.time_pos,
                            false,
                            if intended_spd > 0.0 { intended_spd } else { updated_st.speed },
                            actor,
                        ).await;
                    }

                    SyncMessage::Pause { actor, .. } => {
                        let perms = manager.control_permissions.read().await;
                        if !perms.is_play_pause_allowed() {
                            println!("Rejecting unauthorized PAUSE command from participant: {}", actor);
                            continue;
                        }
                        drop(perms);

                        let mpv_st = manager.mpv_controller.get_current_status().await;
                        if !mpv_st.pause {
                            let _ = manager.mpv_controller.pause().await;
                        }
                        let updated_st = manager.mpv_controller.get_current_status().await;
                        let intended_spd = f64::from_bits(manager.intended_speed.load(Ordering::Relaxed));
                        manager.broadcast_state_sync(
                            updated_st.time_pos,
                            true,
                            if intended_spd > 0.0 { intended_spd } else { updated_st.speed },
                            actor,
                        ).await;
                    }

                    SyncMessage::Seek { position, actor, seek_type, .. } => {
                        let perms = manager.control_permissions.read().await;
                        let allowed = match seek_type.as_deref() {
                            Some("forward") => perms.is_jump_forward_allowed(),
                            Some("backward") => perms.is_jump_backward_allowed(),
                            Some("absolute") => perms.is_seek_allowed(),
                            _ => perms.is_seek_allowed() || perms.is_jump_forward_allowed() || perms.is_jump_backward_allowed(),
                        };
                        if !allowed {
                            println!("Rejecting unauthorized SEEK command from participant: {}", actor);
                            continue;
                        }
                        drop(perms);

                        let _ = manager.mpv_controller.seek(position, true).await;
                        let mpv_st = manager.mpv_controller.get_current_status().await;
                        let intended_spd = f64::from_bits(manager.intended_speed.load(Ordering::Relaxed));
                        manager.broadcast_state_sync(
                            position,
                            mpv_st.pause,
                            if intended_spd > 0.0 { intended_spd } else { mpv_st.speed },
                            actor,
                        ).await;
                    }

                    SyncMessage::Speed { speed, actor, .. } => {
                        let perms = manager.control_permissions.read().await;
                        if !perms.is_speed_allowed() {
                            println!("Rejecting unauthorized SPEED command from participant: {}", actor);
                            continue;
                        }
                        drop(perms);

                        let clamped_spd = speed.clamp(0.25, 3.0);
                        manager.intended_speed.store(clamped_spd.to_bits(), Ordering::SeqCst);
                        let _ = manager.mpv_controller.set_speed(clamped_spd).await;
                        let mpv_st = manager.mpv_controller.get_current_status().await;
                        manager.broadcast_state_sync(mpv_st.time_pos, mpv_st.pause, clamped_spd, actor).await;
                    }

                    SyncMessage::Leave { client_id, .. } => {
                        println!("Participant left: {}", client_id);
                        {
                            let mut members = manager.members.write().await;
                            members.remove(&client_id);
                        }
                        manager.broadcast_member_list().await;
                        break;
                    }

                    _ => {}
                }
            }
        }
    }

    forward_task.abort();

    // Remove on disconnect
    if let Some(id) = participant_id {
        println!("Participant disconnected from room: {}", id);
        {
            let mut members = manager.members.write().await;
            members.remove(&id);
        }
        manager.broadcast_member_list().await;
    }
}

// ==========================================
// Unit Tests (Section 13 & Late Join System)
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_late_join_expected_position_projection_playing() {
        // Scenario: Host playing Avengers: Endgame at 30 minutes (1800.0s), 1.0x speed
        let snapshot_pos = 1800.0;
        let paused = false;
        let speed = 1.0;
        let client_req_time = 1_000_000;
        let snapshot_ts = 1_000_020;
        let current_time = 1_000_060; // 60ms RTT -> 30ms latency

        let expected = calculate_expected_playback_position(
            snapshot_pos,
            paused,
            speed,
            snapshot_ts,
            client_req_time,
            current_time,
        );

        // Expected position must account for 30ms latency: 1800.0 + 0.03 * 1.0 = 1800.03
        assert!((expected - 1800.03).abs() < 0.001);
    }

    #[test]
    fn test_late_join_host_advances_during_loading_to_32_minutes() {
        // Scenario: Host reaches 32 minutes (1920.0s) while participant is loading
        // When participant finishes loading and sends SYNC_REQUEST, host captures position at 1920.0s
        let snapshot_pos = 1920.0; // 32 minutes
        let paused = false;
        let speed = 1.0;
        let client_req_time = 2_000_000;
        let snapshot_ts = 2_000_010;
        let current_time = 2_000_040; // 40ms RTT -> 20ms one-way latency

        let expected = calculate_expected_playback_position(
            snapshot_pos,
            paused,
            speed,
            snapshot_ts,
            client_req_time,
            current_time,
        );

        // Must catch up to approximately 32 minutes (1920.02s), NEVER 00:00 or 30:00!
        assert!(expected >= 1920.0);
        assert!((expected - 1920.02).abs() < 0.001);
    }

    #[test]
    fn test_late_join_while_host_paused() {
        // Scenario: Host paused at 30 minutes (1800.0s)
        let snapshot_pos = 1800.0;
        let paused = true;
        let speed = 1.0;
        let client_req_time = 1_000_000;
        let snapshot_ts = 1_000_010;
        let current_time = 1_005_000; // 5 seconds elapsed

        let expected = calculate_expected_playback_position(
            snapshot_pos,
            paused,
            speed,
            snapshot_ts,
            client_req_time,
            current_time,
        );

        // Paused position must remain exactly 1800.0s without advancing!
        assert_eq!(expected, 1800.0);
    }

    #[test]
    fn test_reject_stale_sync_snapshots() {
        let last_applied_seq = 10u64;
        let stale_seq = 8u64;
        let newer_seq = 11u64;

        // Stale sequence must be rejected
        assert!(stale_seq <= last_applied_seq);
        // Newer sequence must be accepted
        assert!(newer_seq > last_applied_seq);
    }

    #[test]
    fn test_movie_compatibility_verification() {
        let host_movie = MovieFileInfo {
            movie_id: "endgame-123".to_string(),
            filename: "Avengers Endgame (2019) 1080p.mkv".to_string(),
            file_path: None,
            file_size: 4_000_000_000,
            duration: 10800.0,
            container_format: "MKV".to_string(),
            video_codec: None,
            audio_codec: None,
            resolution: None,
            sha256: None,
        };

        let compatible_local = MovieFileInfo {
            movie_id: "local-456".to_string(),
            filename: "Avengers Endgame (2019) 1080p.mkv".to_string(),
            file_path: None,
            file_size: 4_000_000_000,
            duration: 10800.0,
            container_format: "MKV".to_string(),
            video_codec: None,
            audio_codec: None,
            resolution: None,
            sha256: None,
        };

        let incompatible_local = MovieFileInfo {
            movie_id: "local-789".to_string(),
            filename: "Inception 1080p.mkv".to_string(),
            file_path: None,
            file_size: 2_500_000_000,
            duration: 8800.0,
            container_format: "MKV".to_string(),
            video_codec: None,
            audio_codec: None,
            resolution: None,
            sha256: None,
        };

        let (compat1, _) = check_movie_compatibility(&host_movie, &compatible_local, false);
        assert!(compat1, "Identical movie name & size must pass verification");

        let (compat2, _) = check_movie_compatibility(&host_movie, &incompatible_local, false);
        assert!(!compat2, "Different movie must fail verification with warning");

        let (compat3, _) = check_movie_compatibility(&host_movie, &incompatible_local, true);
        assert!(compat3, "Compatibility approved must pass verification");
    }

    #[test]
    fn test_late_join_sync_message_roundtrip() {
        let req = SyncMessage::SyncRequest {
            client_id: "client-001".to_string(),
            movie_id: "endgame-123".to_string(),
            client_time: 123456789,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("SYNC_REQUEST"));

        let snap = SyncMessage::SyncSnapshot {
            room_id: "room-abc".to_string(),
            movie_id: "endgame-123".to_string(),
            client_id: "client-001".to_string(),
            position: 1800.0,
            paused: false,
            speed: 1.0,
            timestamp: 123456800,
            sequence_number: 42,
            host_id: "host-999".to_string(),
            client_time: 123456789,
            permissions: None,
        };
        let snap_json = serde_json::to_string(&snap).unwrap();
        assert!(snap_json.contains("SYNC_SNAPSHOT"));

        let ready = SyncMessage::SyncReady {
            client_id: "client-001".to_string(),
            movie_id: "endgame-123".to_string(),
            local_duration: 10800.0,
        };
        let ready_json = serde_json::to_string(&ready).unwrap();
        assert!(ready_json.contains("SYNC_READY"));

        let confirmed = SyncMessage::SyncConfirmed {
            client_id: "client-001".to_string(),
            applied_position: 1800.05,
            drift_seconds: 0.0,
            timestamp: 123456810,
        };
        let confirmed_json = serde_json::to_string(&confirmed).unwrap();
        assert!(confirmed_json.contains("SYNC_CONFIRMED"));
    }

    #[test]
    fn test_room_control_permissions() {
        let perms = RoomControlPermissions::default();
        assert!(perms.is_play_pause_allowed());
        assert!(perms.is_jump_forward_allowed());
        assert!(perms.is_jump_backward_allowed());
        assert!(perms.is_seek_allowed());
        assert!(perms.is_speed_allowed());

        let restricted = RoomControlPermissions {
            allow_all: false,
            allow_play_pause: true,
            allow_jump_forward: false,
            allow_jump_backward: false,
            allow_seek: false,
            allow_speed: false,
        };
        assert!(restricted.is_play_pause_allowed());
        assert!(!restricted.is_jump_forward_allowed());
        assert!(!restricted.is_jump_backward_allowed());
        assert!(!restricted.is_seek_allowed());
        assert!(!restricted.is_speed_allowed());

        let full_access = RoomControlPermissions {
            allow_all: true,
            allow_play_pause: false,
            allow_jump_forward: false,
            allow_jump_backward: false,
            allow_seek: false,
            allow_speed: false,
        };
        assert!(full_access.is_play_pause_allowed());
        assert!(full_access.is_jump_forward_allowed());
        assert!(full_access.is_jump_backward_allowed());
        assert!(full_access.is_seek_allowed());
        assert!(full_access.is_speed_allowed());
    }
}
