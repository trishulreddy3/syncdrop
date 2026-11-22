//! Peer Discovery Engine for SyncDrop
//! Specifications: PROJECT_SPEC.md Section 3.1 & Phase 3
//! - UDP Multicast: 224.0.0.167:53317
//! - UDP Broadcast fallback on local subnets + 255.255.255.255:53317
//! - 15-second heartbeat freshness & stale device tracking
//! - Multi-interface enumeration & safe malformed packet handling

use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use uuid::Uuid;

pub const DISCOVERY_MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 167);
pub const DISCOVERY_PORT: u16 = 53317;
pub const STALE_TIMEOUT_MS: u64 = 15_000;
pub const REMOVE_TIMEOUT_MS: u64 = 60_000;

/// Active watch room announcement payload carried inside peer discovery
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WatchRoomAnnouncement {
    pub room_id: String,
    pub title: String,
    pub host_alias: String,
    pub port: u16,
    pub movie_title: String,
    pub member_count: usize,
}

/// Wire announcement payload broadcast over LAN
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryAnnouncement {
    pub id: String,
    pub alias: String,
    pub device_model: String,
    pub device_type: String,
    pub port: u16,
    pub protocol: String,
    #[serde(default)]
    pub watch_room: Option<WatchRoomAnnouncement>,
}

/// Discovered peer representation for UI and transfer negotiation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerDevice {
    pub id: String,
    pub alias: String,
    pub device_model: String,
    pub device_type: String,
    pub ip: String,
    pub port: u16,
    pub protocol: String,
    pub last_seen: u64, // Unix ms
    pub is_stale: bool,
    #[serde(default)]
    pub watch_room: Option<WatchRoomAnnouncement>,
}

/// Local device information exposed to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDeviceInfo {
    pub id: String,
    pub alias: String,
    pub device_model: String,
    pub device_type: String,
    pub ip: String,
    pub port: u16,
    pub protocol: String,
}

/// Discovery Manager and Peer Registry
pub struct DiscoveryState {
    pub local_id: String,
    pub local_alias: Arc<RwLock<String>>,
    pub local_device_model: String,
    pub local_device_type: String,
    pub port: Arc<AtomicU64>,
    pub peers: Arc<RwLock<HashMap<String, PeerDevice>>>,
    pub is_running: Arc<AtomicBool>,
    pub active_watch_room: Arc<RwLock<Option<WatchRoomAnnouncement>>>,
}

impl DiscoveryState {
    pub fn new(port: u16) -> Self {
        let local_id = Uuid::new_v4().to_string();
        let pc_name = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "Alex-Desktop-PC".to_string());
        let device_model = format!("Windows Desktop ({})", pc_name);

        Self {
            local_id,
            local_alias: Arc::new(RwLock::new(pc_name)),
            local_device_model: device_model,
            local_device_type: "desktop".to_string(),
            port: Arc::new(AtomicU64::new(port as u64)),
            peers: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(AtomicBool::new(false)),
            active_watch_room: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn set_active_watch_room(&self, room: Option<WatchRoomAnnouncement>) {
        let mut wr = self.active_watch_room.write().await;
        *wr = room;
    }

    pub async fn get_active_watch_room(&self) -> Option<WatchRoomAnnouncement> {
        self.active_watch_room.read().await.clone()
    }

    pub fn set_port(&self, port: u16) {
        self.port.store(port as u64, Ordering::SeqCst);
    }

    pub async fn get_peers(&self) -> Vec<PeerDevice> {
        let map = self.peers.read().await;
        map.values().cloned().collect()
    }

    pub async fn set_alias(&self, new_alias: String) {
        let mut alias = self.local_alias.write().await;
        *alias = new_alias;
    }

    pub async fn get_local_info(&self) -> LocalDeviceInfo {
        let alias = self.local_alias.read().await.clone();
        let ip = get_primary_local_ip();

        LocalDeviceInfo {
            id: self.local_id.clone(),
            alias,
            device_model: self.local_device_model.clone(),
            device_type: self.local_device_type.clone(),
            ip,
            port: self.port.load(Ordering::Relaxed) as u16,
            protocol: "http".to_string(),
        }
    }

    pub async fn clear_peers(&self) {
        let mut map = self.peers.write().await;
        map.clear();
    }
}

/// Helper to get all broadcast targets from active interfaces
pub fn get_broadcast_targets() -> Vec<Ipv4Addr> {
    let mut targets = Vec::new();
    targets.push(Ipv4Addr::new(255, 255, 255, 255));

    if let Ok(interfaces) = NetworkInterface::show() {
        for iface in interfaces {
            if !iface.internal {
                for addr in iface.addr {
                    if let Addr::V4(v4) = addr {
                        if !v4.ip.is_loopback() && !v4.ip.is_link_local() {
                            if let Some(bcast) = v4.broadcast {
                                if !targets.contains(&bcast) {
                                    targets.push(bcast);
                                }
                            } else if let Some(mask) = v4.netmask {
                                let ip_u32 = u32::from(v4.ip);
                                let mask_u32 = u32::from(mask);
                                let bcast = Ipv4Addr::from(ip_u32 | (!mask_u32));
                                if !targets.contains(&bcast) {
                                    targets.push(bcast);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    targets
}

/// Detects the active physical LAN/Wi-Fi IPv4 address
pub fn get_primary_local_ip() -> String {
    if let Ok(interfaces) = NetworkInterface::show() {
        let mut candidates = Vec::new();
        for iface in interfaces {
            if !iface.internal {
                let name_lower = iface.name.to_lowercase();
                let is_physical = name_lower.contains("wi-fi")
                    || name_lower.contains("wifi")
                    || name_lower.contains("ethernet")
                    || name_lower.contains("wlan");

                for addr in iface.addr {
                    if let Addr::V4(v4) = addr {
                        let ip = v4.ip;
                        if !ip.is_loopback() && !ip.is_link_local() {
                            let octets = ip.octets();
                            let is_private_lan = octets[0] == 192 && octets[1] == 168;
                            candidates.push((ip, is_physical, is_private_lan));
                        }
                    }
                }
            }
        }

        // Priority 1: Physical adapter with 192.168.x.x
        if let Some((ip, _, _)) = candidates.iter().find(|(_, phys, lan)| *phys && *lan) {
            return ip.to_string();
        }
        // Priority 2: Any physical adapter
        if let Some((ip, _, _)) = candidates.iter().find(|(_, phys, _)| *phys) {
            return ip.to_string();
        }
        // Priority 3: Any non-loopback candidate
        if let Some((ip, _, _)) = candidates.first() {
            return ip.to_string();
        }
    }
    "127.0.0.1".to_string()
}

/// Constructs a UDP socket bound to 0.0.0.0:53317 with SO_REUSEADDR and joined to multicast group
fn create_multicast_listener_socket(port: u16) -> std::io::Result<tokio::net::UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    socket.set_broadcast(true)?;
    socket.set_nonblocking(true)?;

    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
    socket.bind(&bind_addr.into())?;

    let std_socket: std::net::UdpSocket = socket.into();
    let tokio_socket = tokio::net::UdpSocket::from_std(std_socket)?;

    // Join multicast on default interface
    let _ = tokio_socket.join_multicast_v4(DISCOVERY_MULTICAST_ADDR, Ipv4Addr::UNSPECIFIED);

    // Join multicast explicitly on each active IPv4 interface
    if let Ok(interfaces) = NetworkInterface::show() {
        for iface in interfaces {
            if !iface.internal {
                for addr in iface.addr {
                    if let Addr::V4(v4) = addr {
                        if !v4.ip.is_loopback() && !v4.ip.is_link_local() {
                            let _ = tokio_socket.join_multicast_v4(DISCOVERY_MULTICAST_ADDR, v4.ip);
                        }
                    }
                }
            }
        }
    }

    Ok(tokio_socket)
}

/// Constructs a UDP sender socket configured for broadcast and multicast
fn create_sender_socket() -> std::io::Result<tokio::net::UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    socket.set_broadcast(true)?;
    socket.set_nonblocking(true)?;

    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
    socket.bind(&bind_addr.into())?;

    let std_socket: std::net::UdpSocket = socket.into();
    let tokio_socket = tokio::net::UdpSocket::from_std(std_socket)?;
    let _ = tokio_socket.set_multicast_ttl_v4(4);

    Ok(tokio_socket)
}

/// Starts the periodic discovery announcer, listener, and stale peer tracker
pub fn start_discovery_tasks(app_handle: AppHandle, state: Arc<DiscoveryState>) {
    if state.is_running.swap(true, Ordering::SeqCst) {
        // Already running
        return;
    }

    println!("Starting SyncDrop LAN Peer Discovery engine on port {}", DISCOVERY_PORT);

    // 1. ANNOUNCER TASK (Periodically sends multicast & broadcast every 2 seconds)
    {
        let state_clone = Arc::clone(&state);
        tauri::async_runtime::spawn(async move {
            let sender = match create_sender_socket() {
                Ok(s) => s,
                Err(err) => {
                    eprintln!("Failed to create UDP discovery sender socket: {}", err);
                    return;
                }
            };

            while state_clone.is_running.load(Ordering::SeqCst) {
                let alias = state_clone.local_alias.read().await.clone();
                let http_port = state_clone.port.load(Ordering::Relaxed) as u16;
                let watch_room = state_clone.active_watch_room.read().await.clone();
                let announcement = DiscoveryAnnouncement {
                    id: state_clone.local_id.clone(),
                    alias,
                    device_model: state_clone.local_device_model.clone(),
                    device_type: state_clone.local_device_type.clone(),
                    port: http_port,
                    protocol: "http".to_string(),
                    watch_room,
                };

                if let Ok(bytes) = serde_json::to_vec(&announcement) {
                    // Send UDP Multicast (224.0.0.167:53317)
                    let multi_target = SocketAddrV4::new(DISCOVERY_MULTICAST_ADDR, DISCOVERY_PORT);
                    let _ = sender.send_to(&bytes, multi_target).await;

                    // Send UDP Broadcast fallback across subnets on DISCOVERY_PORT (53317)
                    for bcast_ip in get_broadcast_targets() {
                        let bcast_target = SocketAddrV4::new(bcast_ip, DISCOVERY_PORT);
                        let _ = sender.send_to(&bytes, bcast_target).await;
                    }
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
            println!("Discovery announcer task stopped.");
        });
    }

    // 2. LISTENER TASK (Receives packets on 0.0.0.0:53317)
    {
        let state_clone = Arc::clone(&state);
        let app_handle_clone = app_handle.clone();

        tauri::async_runtime::spawn(async move {
            let listener = match create_multicast_listener_socket(DISCOVERY_PORT) {
                Ok(s) => s,
                Err(err) => {
                    eprintln!("Failed to bind UDP multicast listener socket: {}", err);
                    return;
                }
            };

            let mut buf = [0u8; 4096];

            while state_clone.is_running.load(Ordering::SeqCst) {
                match listener.recv_from(&mut buf).await {
                    Ok((len, src_addr)) => {
                        if len == 0 || len > 4096 {
                            continue;
                        }

                        // Safely parse JSON announcement (ignores malformed/unrelated packets)
                        let announcement: DiscoveryAnnouncement = match serde_json::from_slice(&buf[..len]) {
                            Ok(msg) => msg,
                            Err(_) => {
                                // Malformed packet safely ignored without panicking
                                continue;
                            }
                        };

                        // Ignore our own echo announcement
                        if announcement.id == state_clone.local_id {
                            continue;
                        }

                        let sender_ip = match src_addr {
                            SocketAddr::V4(v4) => v4.ip().to_string(),
                            SocketAddr::V6(v6) => v6.ip().to_string(),
                        };

                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        let peer = PeerDevice {
                            id: announcement.id.clone(),
                            alias: announcement.alias,
                            device_model: announcement.device_model,
                            device_type: announcement.device_type,
                            ip: sender_ip,
                            port: announcement.port,
                            protocol: announcement.protocol,
                            last_seen: now,
                            is_stale: false,
                            watch_room: announcement.watch_room,
                        };

                        let mut peers_guard = state_clone.peers.write().await;
                        let mut should_emit = false;

                        if let Some(existing) = peers_guard.get_mut(&peer.id) {
                            if existing.alias != peer.alias
                                || existing.ip != peer.ip
                                || existing.port != peer.port
                                || existing.device_model != peer.device_model
                                || existing.device_type != peer.device_type
                                || existing.protocol != peer.protocol
                                || existing.watch_room != peer.watch_room
                                || existing.is_stale
                            {
                                *existing = peer;
                                should_emit = true;
                            } else {
                                // Update last_seen timestamp in-place without triggering webview IPC serialization
                                existing.last_seen = now;
                            }
                        } else {
                            // New peer discovered
                            peers_guard.insert(peer.id.clone(), peer);
                            should_emit = true;
                        }

                        if should_emit {
                            let peer_list: Vec<PeerDevice> = peers_guard.values().cloned().collect();
                            drop(peers_guard);
                            // Broadcast update event to React frontend
                            let _ = app_handle_clone.emit("syncdrop://peers-updated", &peer_list);
                        } else {
                            drop(peers_guard);
                        }
                    }
                    Err(e) => {
                        if state_clone.is_running.load(Ordering::SeqCst) {
                            eprintln!("UDP discovery listener error: {}", e);
                            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                        }
                    }
                }
            }
            println!("Discovery listener task stopped.");
        });
    }

    // 3. STALE PEER CLEANER TASK (Checks freshness every 2 seconds)
    {
        let state_clone = Arc::clone(&state);
        let app_handle_clone = app_handle.clone();

        tauri::async_runtime::spawn(async move {
            while state_clone.is_running.load(Ordering::SeqCst) {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                let mut changed = false;
                let mut peers_guard = state_clone.peers.write().await;

                // Remove peers not seen for > 60 seconds
                peers_guard.retain(|_, peer| {
                    let elapsed = now.saturating_sub(peer.last_seen);
                    if elapsed > REMOVE_TIMEOUT_MS {
                        changed = true;
                        false
                    } else {
                        true
                    }
                });

                // Mark stale if not seen for > 15 seconds
                for peer in peers_guard.values_mut() {
                    let elapsed = now.saturating_sub(peer.last_seen);
                    if elapsed > STALE_TIMEOUT_MS && !peer.is_stale {
                        peer.is_stale = true;
                        changed = true;
                    }
                }

                if changed {
                    let peer_list: Vec<PeerDevice> = peers_guard.values().cloned().collect();
                    drop(peers_guard);
                    let _ = app_handle_clone.emit("syncdrop://peers-updated", &peer_list);
                }
            }
            println!("Discovery stale cleaner task stopped.");
        });
    }
}

/// Stops the background discovery tasks
pub fn stop_discovery_tasks(state: Arc<DiscoveryState>) {
    state.is_running.store(false, Ordering::SeqCst);
}
