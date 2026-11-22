import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  SystemInfo,
  PeerDevice,
  LocalDeviceInfo,
  ActiveTransfer,
  IncomingTransferPrompt,
  WatchRoomState,
  StagedFile,
  MpvPlaybackStatus,
  RoomMember,
  MovieFileInfo,
  DiscoveredRoom,
} from "../types";

/**
 * Invoke backend command to get system information
 */
export async function getSystemInfo(): Promise<SystemInfo> {
  return await invoke<SystemInfo>("get_system_info");
}

// ==========================================
// 1. Peer Discovery Commands & Events
// ==========================================

export async function getDiscoveredPeers(): Promise<PeerDevice[]> {
  return await invoke<PeerDevice[]>("get_discovered_peers");
}

export async function getLocalDeviceInfo(): Promise<LocalDeviceInfo> {
  return await invoke<LocalDeviceInfo>("get_local_device_info");
}

export async function setLocalAlias(alias: string): Promise<void> {
  await invoke("set_local_alias", { alias });
}

export async function refreshPeerDiscovery(): Promise<PeerDevice[]> {
  return await invoke<PeerDevice[]>("refresh_peer_discovery");
}

export async function startDiscovery(): Promise<void> {
  await invoke("start_discovery");
}

export async function stopDiscovery(): Promise<void> {
  await invoke("stop_discovery");
}

export async function onPeersUpdated(
  callback: (peers: PeerDevice[]) => void
): Promise<UnlistenFn> {
  return await listen<PeerDevice[]>("syncdrop://peers-updated", (event) => {
    callback(event.payload);
  });
}

export async function onPeerDiscovered(
  callback: (peer: PeerDevice) => void
): Promise<UnlistenFn> {
  return await listen<PeerDevice>("syncdrop://peer-discovered", (event) => {
    callback(event.payload);
  });
}

// ==========================================
// 2. Direct LAN File Transfer Commands & Events
// ==========================================

/**
 * Resolves file metadata (size, basename) for selected absolute file paths on disk
 */
export async function resolveStagedFiles(paths: string[]): Promise<StagedFile[]> {
  return await invoke<StagedFile[]>("resolve_staged_files", { paths });
}

/**
 * Dispatches file transfers to one or multiple selected recipients concurrently
 */
export async function sendFilesToRecipients(
  filePaths: string[],
  recipientIds: string[]
): Promise<void> {
  await invoke("send_files_to_recipients", { filePaths, recipientIds });
}

/**
 * Responds to an incoming transfer prompt (accept = true, decline = false)
 */
export async function respondToTransferPrompt(
  sessionId: string,
  accept: boolean
): Promise<void> {
  await invoke("respond_to_transfer_prompt", { sessionId, accept });
}

/**
 * Aborts / cancels an active transfer session
 */
export async function cancelTransfer(sessionId: string): Promise<void> {
  await invoke("cancel_transfer", { sessionId });
}

/**
 * Retrieves the current local download destination folder
 */
export async function getDownloadDirectory(): Promise<string> {
  return await invoke<string>("get_download_directory");
}

/**
 * Updates the local download destination folder
 */
export async function setDownloadDirectory(path: string): Promise<void> {
  await invoke("set_download_directory", { path });
}

/**
 * Opens the download directory in Windows File Explorer
 */
export async function openDownloadFolder(): Promise<void> {
  await invoke("open_download_folder");
}

/**
 * Opens native Windows file dialog to select multiple files
 */
export async function pickFilesNative(): Promise<string[] | null> {
  try {
    const selected = await open({
      multiple: true,
      title: "Select Files to Send via SyncDrop",
    });
    if (!selected) return null;
    return Array.isArray(selected) ? selected : [selected];
  } catch (err) {
    console.warn("Native file picker failed or cancelled:", err);
    return null;
  }
}

/**
 * Opens native Windows folder picker to change download directory
 */
export async function pickDownloadDirectoryNative(): Promise<string | null> {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select SyncDrop Download Folder",
    });
    if (!selected || Array.isArray(selected)) return null;
    return selected;
  } catch (err) {
    console.warn("Native directory picker failed or cancelled:", err);
    return null;
  }
}

/**
 * Subscribes to real-time byte progress events (both incoming and outgoing)
 */
export async function onTransferProgress(
  callback: (transfer: ActiveTransfer) => void
): Promise<UnlistenFn> {
  return await listen<ActiveTransfer>("syncdrop://transfer-progress", (event) => {
    callback(event.payload);
  });
}

/**
 * Subscribes to incoming transfer prompts from peers
 */
export async function onIncomingTransfer(
  callback: (prompt: IncomingTransferPrompt) => void
): Promise<UnlistenFn> {
  return await listen<IncomingTransferPrompt>("syncdrop://incoming-transfer", (event) => {
    callback(event.payload);
  });
}

// ==========================================
// 3. mpv Media Engine Commands & Events
// ==========================================

export async function checkMpvBinary(): Promise<boolean> {
  return await invoke<boolean>("check_mpv_binary");
}

export async function getMpvBinaryPath(): Promise<string> {
  return await invoke<string>("get_mpv_binary_path");
}

export async function launchMpv(filePath?: string): Promise<void> {
  await invoke("launch_mpv", { filePath });
}

export async function playMpv(): Promise<void> {
  await invoke("play_mpv");
}

export async function pauseMpv(): Promise<void> {
  await invoke("pause_mpv");
}

export async function togglePauseMpv(): Promise<void> {
  await invoke("toggle_pause_mpv");
}

export async function seekMpv(seconds: number, absolute: boolean = true): Promise<void> {
  await invoke("seek_mpv", { seconds, absolute });
}

export async function setMpvSpeed(speed: number): Promise<void> {
  await invoke("set_mpv_speed", { speed });
}

export async function loadMpvFile(path: string): Promise<void> {
  await invoke("load_mpv_file", { path });
}

export async function stopMpv(): Promise<void> {
  await invoke("stop_mpv");
}

export async function getMpvStatus(): Promise<MpvPlaybackStatus> {
  return await invoke<MpvPlaybackStatus>("get_mpv_status");
}

export async function onMpvStatusChanged(
  callback: (status: MpvPlaybackStatus) => void
): Promise<UnlistenFn> {
  return await listen<MpvPlaybackStatus>("syncdrop://mpv-status-changed", (event) => {
    callback(event.payload);
  });
}

/**
 * Opens native file picker filtered for video formats
 */
export async function pickMovieFileNative(): Promise<string | null> {
  try {
    const selected = await open({
      multiple: false,
      title: "Select Movie File for Watch Party",
      filters: [
        {
          name: "Video Files (*.mp4, *.mkv, *.avi, *.mov, *.webm)",
          extensions: ["mp4", "mkv", "avi", "mov", "webm", "m4v"],
        },
        {
          name: "All Files (*.*)",
          extensions: ["*"],
        },
      ],
    });
    if (!selected || Array.isArray(selected)) return null;
    return selected;
  } catch (err) {
    console.warn("Native movie file picker failed or cancelled:", err);
    return null;
  }
}

// ==========================================
// 4. Watch Party & Synchronization Events & Commands
// ==========================================

export async function hostWatchRoom(
  title: string,
  movieInfo?: MovieFileInfo
): Promise<WatchRoomState> {
  return await invoke<WatchRoomState>("host_watch_room", {
    title,
    movieInfo: movieInfo || null,
  });
}

export async function joinWatchRoom(hostUri: string): Promise<void> {
  await invoke("join_watch_room", { hostUri });
}

export async function leaveWatchRoom(): Promise<void> {
  await invoke("leave_watch_room");
}

export async function getWatchRoomState(): Promise<WatchRoomState> {
  return await invoke<WatchRoomState>("get_watch_room_state");
}

export async function dispatchRoomPlay(): Promise<void> {
  await invoke("dispatch_room_play");
}

export async function dispatchRoomPause(): Promise<void> {
  await invoke("dispatch_room_pause");
}

export async function dispatchRoomSeek(
  seconds: number,
  seekType?: "forward" | "backward" | "absolute"
): Promise<void> {
  await invoke("dispatch_room_seek", { seconds, seekType: seekType || null });
}

export async function setRoomPermissions(
  permissions: import("../types").RoomControlPermissions
): Promise<void> {
  await invoke("set_room_permissions", { permissions });
}

export async function dispatchRoomSpeed(speed: number): Promise<void> {
  await invoke("dispatch_room_speed", { speed });
}

export async function setRoomMovie(movieInfo: MovieFileInfo): Promise<void> {
  await invoke("set_room_movie", { movieInfo });
}

export async function setLocalWatchMovie(movieInfo: MovieFileInfo): Promise<void> {
  await invoke("set_local_watch_movie", { movieInfo });
}

export async function syncWatchRoom(): Promise<void> {
  await invoke("sync_watch_room");
}

export async function getRoomMembers(): Promise<RoomMember[]> {
  return await invoke<RoomMember[]>("get_room_members");
}

export async function getDiscoveredRooms(): Promise<DiscoveredRoom[]> {
  return await invoke<DiscoveredRoom[]>("get_discovered_rooms");
}

export async function inspectMovieFile(filePath: string): Promise<MovieFileInfo> {
  return await invoke<MovieFileInfo>("inspect_movie_file", { filePath });
}

export async function getMovieVerification(): Promise<import("../types").MovieVerificationResult> {
  return await invoke<import("../types").MovieVerificationResult>("get_movie_verification");
}

export async function setCompatibilityMode(
  approved: boolean
): Promise<import("../types").MovieVerificationResult> {
  return await invoke<import("../types").MovieVerificationResult>("set_compatibility_mode", { approved });
}

export async function onRoomStateSync(
  callback: (state: WatchRoomState) => void
): Promise<UnlistenFn> {
  return await listen<WatchRoomState>("syncdrop://room-state-sync", (event) => {
    callback(event.payload);
  });
}

export async function onRoomMembersUpdated(
  callback: (members: RoomMember[]) => void
): Promise<UnlistenFn> {
  return await listen<RoomMember[]>("syncdrop://room-members-updated", (event) => {
    callback(event.payload);
  });
}

export async function onRoomFileInfo(
  callback: (info: MovieFileInfo) => void
): Promise<UnlistenFn> {
  return await listen<MovieFileInfo>("syncdrop://room-file-info", (event) => {
    callback(event.payload);
  });
}

export async function onDiscoveredRoomsUpdated(
  callback: (rooms: DiscoveredRoom[]) => void
): Promise<UnlistenFn> {
  return await listen<DiscoveredRoom[]>("syncdrop://discovered-rooms-updated", (event) => {
    callback(event.payload);
  });
}

export async function onRoomDisconnected(
  callback: () => void
): Promise<UnlistenFn> {
  return await listen("syncdrop://room-disconnected", () => {
    callback();
  });
}
