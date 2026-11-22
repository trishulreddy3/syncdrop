//! Direct LAN File Transfer Module (Streaming HTTP)
//! Specifications: PROJECT_SPEC.md Section 3.2 & Phase 4
//! - Direct TCP file transfer using Axum HTTP server & Reqwest HTTP client
//! - Large-file streaming (zero whole-file RAM buffering)
//! - Multi-recipient independent tracking and isolation
//! - Strict filename sanitization and anti-path-traversal
//! - Anti-overwrite auto-versioning (e.g. "Video (1).mp4")
//! - Live real-time transfer progress, speed, and status via Tauri events
//! - Transfer prompt modal acceptance / decline handshake
//! - Safe partial file cleanup on connection failure or cancellation

use axum::{
    body::Body,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::{oneshot, RwLock};
use tokio_util::io::ReaderStream;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use crate::discovery::{DiscoveryState, PeerDevice};

pub const DEFAULT_TRANSFER_PORT: u16 = 53317;
pub const FALLBACK_TRANSFER_PORT: u16 = 53319;
pub const CHUNK_BUFFER_SIZE: usize = 64 * 1024; // 64 KB streaming chunks

// ==========================================
// Data Transfer Models
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMetadata {
    pub file_id: String,
    pub filename: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedFileInfo {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingTransferPrompt {
    pub transfer_id: String,
    pub sender_id: String,
    pub sender_alias: String,
    pub sender_device_model: String,
    pub sender_ip: String,
    pub file: FileMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareUploadRequest {
    pub session_id: String,
    pub sender_id: String,
    pub sender_alias: String,
    pub sender_device_model: String,
    pub sender_ip: String,
    pub files: Vec<FileMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareUploadResponse {
    pub status: String, // "accepted", "declined", "error"
    pub token: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadQueryParams {
    pub session_id: String,
    pub file_id: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveTransfer {
    pub transfer_id: String,
    pub file_id: String,
    pub filename: String,
    pub total_bytes: u64,
    pub bytes_transferred: u64,
    pub speed_bytes_per_sec: u64,
    pub status: String, // "pending" | "waiting_response" | "transferring" | "completed" | "failed" | "declined" | "cancelled"
    pub direction: String, // "incoming" | "outgoing"
    pub peer_id: String,
    pub peer_alias: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved_path: Option<String>,
}

// ==========================================
// Accepted Session State
// ==========================================

#[derive(Clone)]
pub struct AcceptedSession {
    pub session_id: String,
    pub token: String,
    pub sender_id: String,
    pub sender_alias: String,
    pub files: Vec<FileMetadata>,
}

pub struct TransferState {
    pub app_handle: Arc<std::sync::RwLock<Option<AppHandle>>>,
    pub download_dir: Arc<RwLock<PathBuf>>,
    pub pending_prompts: Arc<RwLock<HashMap<String, oneshot::Sender<bool>>>>,
    pub accepted_sessions: Arc<RwLock<HashMap<String, AcceptedSession>>>,
    pub active_cancellations: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
    pub bound_port: Arc<AtomicU64>,
}

impl TransferState {
    pub fn new() -> Self {
        let download_dir = resolve_default_download_directory();
        Self {
            app_handle: Arc::new(std::sync::RwLock::new(None)),
            download_dir: Arc::new(RwLock::new(download_dir)),
            pending_prompts: Arc::new(RwLock::new(HashMap::new())),
            accepted_sessions: Arc::new(RwLock::new(HashMap::new())),
            active_cancellations: Arc::new(RwLock::new(HashMap::new())),
            bound_port: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        if let Ok(mut lock) = self.app_handle.write() {
            *lock = Some(handle);
        }
    }

    pub fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) {
        if let Ok(lock) = self.app_handle.read() {
            if let Some(handle) = lock.as_ref() {
                let _ = handle.emit(event, payload);
            }
        }
    }
}

// ==========================================
// Path & Filename Sanitization Utilities
// ==========================================

/// Resolves standard Windows default download folder: `%USERPROFILE%\Downloads\SyncDrop`
pub fn resolve_default_download_directory() -> PathBuf {
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        let p = PathBuf::from(userprofile).join("Downloads").join("SyncDrop");
        let _ = std::fs::create_dir_all(&p);
        return p;
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join("Downloads").join("SyncDrop");
        let _ = std::fs::create_dir_all(&p);
        return p;
    }
    let fallback = PathBuf::from("./SyncDrop_Downloads");
    let _ = std::fs::create_dir_all(&fallback);
    fallback
}

/// Strict filename sanitization to eliminate path traversal and illegal Windows characters
pub fn sanitize_filename(filename: &str) -> String {
    let raw_name = Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed_file");

    // Remove illegal Windows characters: < > : " / \ | ? * and control chars
    let mut clean: String = raw_name
        .chars()
        .filter(|c| !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0'..='\x1f'))
        .collect();

    clean = clean.trim().to_string();

    if clean.is_empty() || clean == "." || clean == ".." {
        clean = format!("file_{}", &Uuid::new_v4().to_string()[..8]);
    }

    // Guard against Windows DOS reserved device names (CON, PRN, AUX, NUL, COM1-9, LPT1-9)
    let stem = Path::new(&clean)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_uppercase();

    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL"
            | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
            | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
    ) {
        clean = format!("syncdrop_{}", clean);
    }

    clean
}

/// Generates a non-conflicting unique destination path: "video.mp4" -> "video (1).mp4"
pub fn generate_unique_destination_path(dir: &Path, clean_filename: &str) -> PathBuf {
    let direct_path = dir.join(clean_filename);
    if !direct_path.exists() {
        return direct_path;
    }

    let stem = Path::new(clean_filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = Path::new(clean_filename)
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    let mut counter = 1;
    loop {
        let candidate_name = format!("{} ({}){}", stem, counter, ext);
        let candidate_path = dir.join(&candidate_name);
        if !candidate_path.exists() {
            return candidate_path;
        }
        counter += 1;
    }
}

// ==========================================
// HTTP Server Handlers (Axum)
// ==========================================

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "SyncDrop Transfer Service Active")
}

/// Endpoint: POST /api/transfer/prepare
/// Handles incoming transfer requests from peers, emits prompt modal event, and awaits user decision
async fn handle_prepare_upload(
    State(state): State<Arc<TransferState>>,
    Json(payload): Json<PrepareUploadRequest>,
) -> impl IntoResponse {
    if payload.files.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(PrepareUploadResponse {
                status: "error".to_string(),
                token: None,
                message: "No files specified in transfer request".to_string(),
            }),
        );
    }

    let session_id = payload.session_id.clone();
    let prompt_file = payload.files[0].clone();

    let prompt = IncomingTransferPrompt {
        transfer_id: session_id.clone(),
        sender_id: payload.sender_id.clone(),
        sender_alias: payload.sender_alias.clone(),
        sender_device_model: payload.sender_device_model.clone(),
        sender_ip: payload.sender_ip.clone(),
        file: prompt_file,
    };

    // Emit event to Tauri frontend to display TransferPromptModal
    state.emit("syncdrop://incoming-transfer", &prompt);

    // Create oneshot channel to await the receiver's UI decision
    let (tx, rx) = oneshot::channel::<bool>();
    {
        let mut prompts = state.pending_prompts.write().await;
        prompts.insert(session_id.clone(), tx);
    }

    // Wait up to 60 seconds for receiver's decision
    match tokio::time::timeout(Duration::from_secs(60), rx).await {
        Ok(Ok(true)) => {
            // User accepted
            let token = Uuid::new_v4().to_string();
            let accepted = AcceptedSession {
                session_id: session_id.clone(),
                token: token.clone(),
                sender_id: payload.sender_id,
                sender_alias: payload.sender_alias,
                files: payload.files,
            };

            {
                let mut sessions = state.accepted_sessions.write().await;
                sessions.insert(session_id, accepted);
            }

            (
                StatusCode::OK,
                Json(PrepareUploadResponse {
                    status: "accepted".to_string(),
                    token: Some(token),
                    message: "Recipient accepted file transfer".to_string(),
                }),
            )
        }
        Ok(Ok(false)) => {
            // User declined
            (
                StatusCode::OK,
                Json(PrepareUploadResponse {
                    status: "declined".to_string(),
                    token: None,
                    message: "Recipient declined file transfer".to_string(),
                }),
            )
        }
        _ => {
            // Timed out or channel closed
            {
                let mut prompts = state.pending_prompts.write().await;
                prompts.remove(&session_id);
            }

            (
                StatusCode::OK,
                Json(PrepareUploadResponse {
                    status: "declined".to_string(),
                    token: None,
                    message: "Transfer request timed out".to_string(),
                }),
            )
        }
    }
}

/// Endpoint: POST /api/transfer/upload
/// Streams incoming file chunks directly to disk without buffering in RAM
async fn handle_upload_file(
    State(state): State<Arc<TransferState>>,
    Query(params): Query<UploadQueryParams>,
    body: Body,
) -> impl IntoResponse {
    // 1. Validate session and acceptance token
    let session_data = {
        let sessions = state.accepted_sessions.read().await;
        sessions.get(&params.session_id).cloned()
    };

    let session = match session_data {
        Some(s) if s.token == params.token => s,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "Invalid or expired transfer authorization token"
                })),
            );
        }
    };

    // 2. Find file metadata
    let file_meta = match session.files.iter().find(|f| f.file_id == params.file_id) {
        Some(m) => m.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "File identifier not associated with this transfer session"
                })),
            );
        }
    };

    // 3. Resolve destination directory and sanitized path
    let download_dir = state.download_dir.read().await.clone();
    if let Err(err) = tokio::fs::create_dir_all(&download_dir).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": format!("Failed to create download directory: {}", err)
            })),
        );
    }

    let clean_filename = sanitize_filename(&file_meta.filename);
    let final_path = generate_unique_destination_path(&download_dir, &clean_filename);
    let temp_filename = format!("{}.{}.syncdrop_part", clean_filename, &Uuid::new_v4().to_string()[..8]);
    let temp_path = download_dir.join(&temp_filename);

    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let mut cancellations = state.active_cancellations.write().await;
        cancellations.insert(params.session_id.clone(), Arc::clone(&cancel_flag));
    }

    // 4. Open temporary output file for streaming writes
    let mut file = match tokio::fs::File::create(&temp_path).await {
        Ok(f) => f,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "status": "error",
                    "message": format!("Could not open file for writing: {}", err)
                })),
            );
        }
    };

    // 5. Stream chunks from body into file
    let mut bytes_received = 0u64;
    let mut last_progress_time = Instant::now();
    let mut last_bytes_count = 0u64;
    let mut current_speed = 0u64;

    let mut stream = body.into_data_stream();

    // Initial progress event
    state.emit(
        "syncdrop://transfer-progress",
        ActiveTransfer {
            transfer_id: params.session_id.clone(),
            file_id: file_meta.file_id.clone(),
            filename: clean_filename.clone(),
            total_bytes: file_meta.size,
            bytes_transferred: 0,
            speed_bytes_per_sec: 0,
            status: "transferring".to_string(),
            direction: "incoming".to_string(),
            peer_id: session.sender_id.clone(),
            peer_alias: session.sender_alias.clone(),
            error_message: None,
            saved_path: None,
        },
    );

    let mut stream_error: Option<String> = None;

    while let Some(chunk_result) = stream.next().await {
        // Check for cancellation
        if cancel_flag.load(Ordering::Relaxed) {
            stream_error = Some("Transfer cancelled by user".to_string());
            break;
        }

        match chunk_result {
            Ok(chunk) => {
                if let Err(err) = file.write_all(&chunk).await {
                    stream_error = Some(format!("Disk write error: {}", err));
                    break;
                }
                bytes_received += chunk.len() as u64;

                let now = Instant::now();
                let elapsed = now.duration_since(last_progress_time);
                if elapsed >= Duration::from_millis(200) {
                    let delta_bytes = bytes_received.saturating_sub(last_bytes_count);
                    let secs = elapsed.as_secs_f64();
                    if secs > 0.0 {
                        current_speed = (delta_bytes as f64 / secs) as u64;
                    }
                    last_progress_time = now;
                    last_bytes_count = bytes_received;

                    state.emit(
                        "syncdrop://transfer-progress",
                        ActiveTransfer {
                            transfer_id: params.session_id.clone(),
                            file_id: file_meta.file_id.clone(),
                            filename: clean_filename.clone(),
                            total_bytes: file_meta.size,
                            bytes_transferred: bytes_received,
                            speed_bytes_per_sec: current_speed,
                            status: "transferring".to_string(),
                            direction: "incoming".to_string(),
                            peer_id: session.sender_id.clone(),
                            peer_alias: session.sender_alias.clone(),
                            error_message: None,
                            saved_path: None,
                        },
                    );
                }
            }
            Err(err) => {
                stream_error = Some(format!("Network stream read error: {}", err));
                break;
            }
        }
    }

    // Cleanup cancellation flag
    {
        let mut cancellations = state.active_cancellations.write().await;
        cancellations.remove(&params.session_id);
    }

    // Flush and close file
    let _ = file.flush().await;
    drop(file);

    // 6. Handle errors or finalize file
    if let Some(err_msg) = stream_error {
        let _ = tokio::fs::remove_file(&temp_path).await;
        state.emit(
            "syncdrop://transfer-progress",
            ActiveTransfer {
                transfer_id: params.session_id.clone(),
                file_id: file_meta.file_id.clone(),
                filename: clean_filename.clone(),
                total_bytes: file_meta.size,
                bytes_transferred: bytes_received,
                speed_bytes_per_sec: 0,
                status: "failed".to_string(),
                direction: "incoming".to_string(),
                peer_id: session.sender_id,
                peer_alias: session.sender_alias,
                error_message: Some(err_msg.clone()),
                saved_path: None,
            },
        );

        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": err_msg
            })),
        );
    }

    // Validate size if specified
    if file_meta.size > 0 && bytes_received != file_meta.size {
        let _ = tokio::fs::remove_file(&temp_path).await;
        let err_msg = format!(
            "Received size mismatch: expected {} bytes, received {} bytes",
            file_meta.size, bytes_received
        );
        state.emit(
            "syncdrop://transfer-progress",
            ActiveTransfer {
                transfer_id: params.session_id.clone(),
                file_id: file_meta.file_id.clone(),
                filename: clean_filename.clone(),
                total_bytes: file_meta.size,
                bytes_transferred: bytes_received,
                speed_bytes_per_sec: 0,
                status: "failed".to_string(),
                direction: "incoming".to_string(),
                peer_id: session.sender_id,
                peer_alias: session.sender_alias,
                error_message: Some(err_msg.clone()),
                saved_path: None,
            },
        );

        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "status": "error",
                "message": err_msg
            })),
        );
    }

    // Rename .syncdrop_part to final destination
    if let Err(err) = tokio::fs::rename(&temp_path, &final_path).await {
        let err_msg = format!("Failed to commit received file: {}", err);
        let _ = tokio::fs::remove_file(&temp_path).await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": err_msg
            })),
        );
    }

    let saved_path_str = final_path.to_string_lossy().to_string();

    // Emit completed event
    state.emit(
        "syncdrop://transfer-progress",
        ActiveTransfer {
            transfer_id: params.session_id,
            file_id: file_meta.file_id,
            filename: clean_filename,
            total_bytes: bytes_received,
            bytes_transferred: bytes_received,
            speed_bytes_per_sec: 0,
            status: "completed".to_string(),
            direction: "incoming".to_string(),
            peer_id: session.sender_id,
            peer_alias: session.sender_alias,
            error_message: None,
            saved_path: Some(saved_path_str.clone()),
        },
    );

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "completed",
            "savedPath": saved_path_str
        })),
    )
}

// ==========================================
// HTTP Server Starter
// ==========================================

/// Starts the Axum HTTP transfer server on 0.0.0.0
pub async fn start_transfer_http_server(
    state: Arc<TransferState>,
    preferred_port: u16,
) -> Result<u16, String> {
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/transfer/prepare", post(handle_prepare_upload))
        .route("/api/transfer/upload", post(handle_upload_file))
        .layer(CorsLayer::permissive())
        .with_state(state.clone());

    // Try binding preferred port first (53317), then fallback to 53319 or 0
    let listener = match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", preferred_port)).await {
        Ok(l) => l,
        Err(e1) => {
            eprintln!(
                "Port {} in use ({}), attempting fallback port {}...",
                preferred_port, e1, FALLBACK_TRANSFER_PORT
            );
            match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", FALLBACK_TRANSFER_PORT)).await {
                Ok(l) => l,
                Err(e2) => {
                    eprintln!(
                        "Fallback port {} in use ({}), binding random available port...",
                        FALLBACK_TRANSFER_PORT, e2
                    );
                    tokio::net::TcpListener::bind("0.0.0.0:0")
                        .await
                        .map_err(|e3| format!("Failed to bind any TCP port for file transfer: {}", e3))?
                }
            }
        }
    };

    let local_addr = listener
        .local_addr()
        .map_err(|e| format!("Could not get local address of listener: {}", e))?;
    let bound_port = local_addr.port();
    state.bound_port.store(bound_port as u64, Ordering::SeqCst);

    println!(
        "SyncDrop HTTP File Transfer service listening on http://0.0.0.0:{}",
        bound_port
    );

    tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, app).await {
            eprintln!("SyncDrop HTTP File Transfer server terminated: {}", err);
        }
    });

    Ok(bound_port)
}

// ==========================================
// Outgoing Transfer Engine (Sender Side)
// ==========================================

/// Resolves real file information from disk paths
pub fn resolve_staged_files(paths: Vec<String>) -> Result<Vec<StagedFileInfo>, String> {
    let mut files = Vec::new();
    for p in paths {
        let path = PathBuf::from(&p);
        if !path.exists() {
            return Err(format!("File does not exist: {}", p));
        }
        let meta = std::fs::metadata(&path)
            .map_err(|e| format!("Could not read file metadata for {}: {}", p, e))?;

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        files.push(StagedFileInfo {
            id: Uuid::new_v4().to_string(),
            name,
            size: meta.len(),
            path: p,
        });
    }
    Ok(files)
}

/// Dispatches direct TCP file transfers to selected recipients concurrently
pub async fn send_files_to_recipients(
    app_handle: AppHandle,
    transfer_state: Arc<TransferState>,
    discovery_state: Arc<DiscoveryState>,
    file_paths: Vec<String>,
    recipient_ids: Vec<String>,
) -> Result<(), String> {
    if file_paths.is_empty() {
        return Err("No files selected to send".to_string());
    }
    if recipient_ids.is_empty() {
        return Err("No recipients selected".to_string());
    }

    // 1. Resolve and validate files from disk
    let staged_files = resolve_staged_files(file_paths)?;

    // 2. Fetch recipient peer records from discovery peer registry
    let all_peers = discovery_state.get_peers().await;
    let local_info = discovery_state.get_local_info().await;

    let recipients: Vec<PeerDevice> = all_peers
        .into_iter()
        .filter(|p| recipient_ids.contains(&p.id))
        .collect();

    if recipients.is_empty() {
        return Err("None of the selected recipients could be located in the peer registry".to_string());
    }

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // 3. For each recipient, spawn an independent Tokio worker task
    // One recipient's decline, delay, or failure will NOT affect any other recipient!
    for recipient in recipients {
        let app_handle_clone = app_handle.clone();
        let client_clone = client.clone();
        let staged_files_clone = staged_files.clone();
        let local_info_clone = local_info.clone();
        let transfer_state_clone = Arc::clone(&transfer_state);

        tokio::spawn(async move {
            transfer_to_single_recipient(
                app_handle_clone,
                transfer_state_clone,
                client_clone,
                recipient,
                staged_files_clone,
                local_info_clone,
            )
            .await;
        });
    }

    Ok(())
}

/// Executes file transfer workflow for one specific recipient
async fn transfer_to_single_recipient(
    app_handle: AppHandle,
    transfer_state: Arc<TransferState>,
    client: reqwest::Client,
    recipient: PeerDevice,
    staged_files: Vec<StagedFileInfo>,
    local_info: crate::discovery::LocalDeviceInfo,
) {
    let session_id = Uuid::new_v4().to_string();
    let recipient_url = format!("http://{}:{}", recipient.ip, recipient.port);

    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let mut cancellations = transfer_state.active_cancellations.write().await;
        cancellations.insert(session_id.clone(), Arc::clone(&cancel_flag));
    }

    // Step 1: Send Prepare Upload request
    let file_metas: Vec<FileMetadata> = staged_files
        .iter()
        .map(|f| FileMetadata {
            file_id: f.id.clone(),
            filename: f.name.clone(),
            size: f.size,
            mime_type: None,
        })
        .collect();

    let prepare_req = PrepareUploadRequest {
        session_id: session_id.clone(),
        sender_id: local_info.id,
        sender_alias: local_info.alias,
        sender_device_model: local_info.device_model,
        sender_ip: local_info.ip,
        files: file_metas.clone(),
    };

    // Emit initial "waiting_response" status to sender UI
    for f in &file_metas {
        let _ = app_handle.emit(
            "syncdrop://transfer-progress",
            ActiveTransfer {
                transfer_id: session_id.clone(),
                file_id: f.file_id.clone(),
                filename: f.filename.clone(),
                total_bytes: f.size,
                bytes_transferred: 0,
                speed_bytes_per_sec: 0,
                status: "waiting_response".to_string(),
                direction: "outgoing".to_string(),
                peer_id: recipient.id.clone(),
                peer_alias: recipient.alias.clone(),
                error_message: None,
                saved_path: None,
            },
        );
    }

    let prepare_url = format!("{}/api/transfer/prepare", recipient_url);
    let prepare_response = match client
        .post(&prepare_url)
        .json(&prepare_req)
        .timeout(Duration::from_secs(65))
        .send()
        .await
    {
        Ok(res) => res,
        Err(err) => {
            // Recipient unreachable or connection refused
            for f in &file_metas {
                let _ = app_handle.emit(
                    "syncdrop://transfer-progress",
                    ActiveTransfer {
                        transfer_id: session_id.clone(),
                        file_id: f.file_id.clone(),
                        filename: f.filename.clone(),
                        total_bytes: f.size,
                        bytes_transferred: 0,
                        speed_bytes_per_sec: 0,
                        status: "failed".to_string(),
                        direction: "outgoing".to_string(),
                        peer_id: recipient.id.clone(),
                        peer_alias: recipient.alias.clone(),
                        error_message: Some(format!("Could not connect to peer: {}", err)),
                        saved_path: None,
                    },
                );
            }
            return;
        }
    };

    let prepare_data: PrepareUploadResponse = match prepare_response.json().await {
        Ok(d) => d,
        Err(err) => {
            for f in &file_metas {
                let _ = app_handle.emit(
                    "syncdrop://transfer-progress",
                    ActiveTransfer {
                        transfer_id: session_id.clone(),
                        file_id: f.file_id.clone(),
                        filename: f.filename.clone(),
                        total_bytes: f.size,
                        bytes_transferred: 0,
                        speed_bytes_per_sec: 0,
                        status: "failed".to_string(),
                        direction: "outgoing".to_string(),
                        peer_id: recipient.id.clone(),
                        peer_alias: recipient.alias.clone(),
                        error_message: Some(format!("Invalid response format: {}", err)),
                        saved_path: None,
                    },
                );
            }
            return;
        }
    };

    if prepare_data.status != "accepted" {
        // Recipient explicitly declined or prompt timed out
        let status = if prepare_data.status == "declined" { "declined" } else { "failed" };
        for f in &file_metas {
            let _ = app_handle.emit(
                "syncdrop://transfer-progress",
                ActiveTransfer {
                    transfer_id: session_id.clone(),
                    file_id: f.file_id.clone(),
                    filename: f.filename.clone(),
                    total_bytes: f.size,
                    bytes_transferred: 0,
                    speed_bytes_per_sec: 0,
                    status: status.to_string(),
                    direction: "outgoing".to_string(),
                    peer_id: recipient.id.clone(),
                    peer_alias: recipient.alias.clone(),
                    error_message: Some(prepare_data.message.clone()),
                    saved_path: None,
                },
            );
        }
        return;
    }

    let token = match prepare_data.token {
        Some(t) => t,
        None => {
            for f in &file_metas {
                let _ = app_handle.emit(
                    "syncdrop://transfer-progress",
                    ActiveTransfer {
                        transfer_id: session_id.clone(),
                        file_id: f.file_id.clone(),
                        filename: f.filename.clone(),
                        total_bytes: f.size,
                        bytes_transferred: 0,
                        speed_bytes_per_sec: 0,
                        status: "failed".to_string(),
                        direction: "outgoing".to_string(),
                        peer_id: recipient.id.clone(),
                        peer_alias: recipient.alias.clone(),
                        error_message: Some("Recipient did not provide an authorization token".to_string()),
                        saved_path: None,
                    },
                );
            }
            return;
        }
    };

    // Step 2: Stream each file to the recipient
    for staged in staged_files {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = app_handle.emit(
                "syncdrop://transfer-progress",
                ActiveTransfer {
                    transfer_id: session_id.clone(),
                    file_id: staged.id.clone(),
                    filename: staged.name.clone(),
                    total_bytes: staged.size,
                    bytes_transferred: 0,
                    speed_bytes_per_sec: 0,
                    status: "cancelled".to_string(),
                    direction: "outgoing".to_string(),
                    peer_id: recipient.id.clone(),
                    peer_alias: recipient.alias.clone(),
                    error_message: Some("Transfer cancelled".to_string()),
                    saved_path: None,
                },
            );
            break;
        }

        let file = match tokio::fs::File::open(&staged.path).await {
            Ok(f) => f,
            Err(err) => {
                let _ = app_handle.emit(
                    "syncdrop://transfer-progress",
                    ActiveTransfer {
                        transfer_id: session_id.clone(),
                        file_id: staged.id.clone(),
                        filename: staged.name.clone(),
                        total_bytes: staged.size,
                        bytes_transferred: 0,
                        speed_bytes_per_sec: 0,
                        status: "failed".to_string(),
                        direction: "outgoing".to_string(),
                        peer_id: recipient.id.clone(),
                        peer_alias: recipient.alias.clone(),
                        error_message: Some(format!("Could not read local file: {}", err)),
                        saved_path: None,
                    },
                );
                continue;
            }
        };

        let upload_url = format!(
            "{}/api/transfer/upload?sessionId={}&fileId={}&token={}",
            recipient_url, session_id, staged.id, token
        );

        let bytes_sent_counter = Arc::new(AtomicU64::new(0));
        let total_size = staged.size;

        // Wrap file in ReaderStream with chunk inspect to track bytes and speed
        let app_handle_progress = app_handle.clone();
        let session_id_progress = session_id.clone();
        let file_id_progress = staged.id.clone();
        let filename_progress = staged.name.clone();
        let recipient_id_progress = recipient.id.clone();
        let recipient_alias_progress = recipient.alias.clone();
        let bytes_sent_for_stream = Arc::clone(&bytes_sent_counter);

        let start_time = Instant::now();
        let last_emit_ms = Arc::new(AtomicU64::new(0));
        let last_bytes = Arc::new(AtomicU64::new(0));

        let reader_stream = ReaderStream::with_capacity(file, CHUNK_BUFFER_SIZE);
        let progress_stream = reader_stream.inspect(move |chunk_result| {
            if let Ok(chunk) = chunk_result {
                let added = chunk.len() as u64;
                let current_sent = bytes_sent_for_stream.fetch_add(added, Ordering::Relaxed) + added;

                let now_ms = start_time.elapsed().as_millis() as u64;
                let last_ms = last_emit_ms.load(Ordering::Relaxed);
                let is_done = current_sent >= total_size;

                if is_done || now_ms.saturating_sub(last_ms) >= 200 {
                    if last_emit_ms.compare_exchange(last_ms, now_ms, Ordering::SeqCst, Ordering::Relaxed).is_ok() || is_done {
                        let prev = last_bytes.swap(current_sent, Ordering::Relaxed);
                        let delta = current_sent.saturating_sub(prev);
                        let elapsed_secs = (now_ms.saturating_sub(last_ms) as f64) / 1000.0;
                        let speed = if elapsed_secs > 0.0 { (delta as f64 / elapsed_secs) as u64 } else { 0 };

                        let app = app_handle_progress.clone();
                        let s_id = session_id_progress.clone();
                        let f_id = file_id_progress.clone();
                        let f_name = filename_progress.clone();
                        let r_id = recipient_id_progress.clone();
                        let r_alias = recipient_alias_progress.clone();

                        tokio::spawn(async move {
                            let _ = app.emit(
                                "syncdrop://transfer-progress",
                                ActiveTransfer {
                                    transfer_id: s_id,
                                    file_id: f_id,
                                    filename: f_name,
                                    total_bytes: total_size,
                                    bytes_transferred: current_sent,
                                    speed_bytes_per_sec: speed,
                                    status: "transferring".to_string(),
                                    direction: "outgoing".to_string(),
                                    peer_id: r_id,
                                    peer_alias: r_alias,
                                    error_message: None,
                                    saved_path: None,
                                },
                            );
                        });
                    }
                }
            }
        });

        let body = reqwest::Body::wrap_stream(progress_stream);

        let upload_res = client
            .post(&upload_url)
            .header(reqwest::header::CONTENT_LENGTH, staged.size)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(body)
            .send()
            .await;

        match upload_res {
            Ok(res) if res.status().is_success() => {
                let _ = app_handle.emit(
                    "syncdrop://transfer-progress",
                    ActiveTransfer {
                        transfer_id: session_id.clone(),
                        file_id: staged.id.clone(),
                        filename: staged.name.clone(),
                        total_bytes: staged.size,
                        bytes_transferred: staged.size,
                        speed_bytes_per_sec: 0,
                        status: "completed".to_string(),
                        direction: "outgoing".to_string(),
                        peer_id: recipient.id.clone(),
                        peer_alias: recipient.alias.clone(),
                        error_message: None,
                        saved_path: None,
                    },
                );
            }
            Ok(res) => {
                let err_text = res.text().await.unwrap_or_else(|_| "Unknown error".into());
                let _ = app_handle.emit(
                    "syncdrop://transfer-progress",
                    ActiveTransfer {
                        transfer_id: session_id.clone(),
                        file_id: staged.id.clone(),
                        filename: staged.name.clone(),
                        total_bytes: staged.size,
                        bytes_transferred: bytes_sent_counter.load(Ordering::Relaxed),
                        speed_bytes_per_sec: 0,
                        status: "failed".to_string(),
                        direction: "outgoing".to_string(),
                        peer_id: recipient.id.clone(),
                        peer_alias: recipient.alias.clone(),
                        error_message: Some(format!("Recipient returned error: {}", err_text)),
                        saved_path: None,
                    },
                );
            }
            Err(err) => {
                let _ = app_handle.emit(
                    "syncdrop://transfer-progress",
                    ActiveTransfer {
                        transfer_id: session_id.clone(),
                        file_id: staged.id.clone(),
                        filename: staged.name.clone(),
                        total_bytes: staged.size,
                        bytes_transferred: bytes_sent_counter.load(Ordering::Relaxed),
                        speed_bytes_per_sec: 0,
                        status: "failed".to_string(),
                        direction: "outgoing".to_string(),
                        peer_id: recipient.id.clone(),
                        peer_alias: recipient.alias.clone(),
                        error_message: Some(format!("Upload stream failed: {}", err)),
                        saved_path: None,
                    },
                );
            }
        }
    }

    // Cleanup cancellation flag for this session
    {
        let mut cancellations = transfer_state.active_cancellations.write().await;
        cancellations.remove(&session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_strips_traversal_and_invalid_chars() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename(r"C:\Windows\System32\cmd.exe"), "cmd.exe");
        assert_eq!(sanitize_filename("invalid:name*with?chars.mp4"), "invalidnamewithchars.mp4");
        assert_eq!(sanitize_filename("CON.txt"), "syncdrop_CON.txt");
        assert_eq!(sanitize_filename("aux.mp4"), "syncdrop_aux.mp4");
    }

    #[test]
    fn test_generate_unique_destination_path() {
        let temp_dir = std::env::temp_dir().join(format!("syncdrop_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let base_name = "test_movie.mp4";
        let path1 = generate_unique_destination_path(&temp_dir, base_name);
        assert_eq!(path1, temp_dir.join("test_movie.mp4"));

        // Create the file so it exists
        std::fs::write(&path1, b"hello").unwrap();

        let path2 = generate_unique_destination_path(&temp_dir, base_name);
        assert_eq!(path2, temp_dir.join("test_movie (1).mp4"));

        // Create the second file as well
        std::fs::write(&path2, b"hello 2").unwrap();

        let path3 = generate_unique_destination_path(&temp_dir, base_name);
        assert_eq!(path3, temp_dir.join("test_movie (2).mp4"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
