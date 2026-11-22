/**
 * SyncDrop Core Type Definitions
 * Source of truth: PROJECT_SPEC.md
 */

// ==========================================
// 1. Peer Discovery Types (Section 3.1)
// ==========================================

export type DeviceType = "desktop" | "mobile" | "server";

export interface PeerDevice {
  id: string;
  alias: string;
  deviceModel: string;
  deviceType: DeviceType;
  ip: string;
  port: number;
  protocol: "http";
  lastSeen: number; // Unix timestamp in ms
  isStale?: boolean;
}

export interface DiscoveryAnnouncement {
  id: string;
  alias: string;
  deviceModel: string;
  deviceType: DeviceType;
  port: number;
  protocol: "http";
}

export interface LocalDeviceInfo {
  id: string;
  alias: string;
  deviceModel: string;
  deviceType: string;
  ip: string;
  port: number;
  protocol: string;
}

// ==========================================
// 2. Direct File Transfer Types (Section 3.2)
// ==========================================

export type TransferStatus =
  | "pending"
  | "waiting_response"
  | "accepted"
  | "declined"
  | "transferring"
  | "completed"
  | "failed"
  | "cancelled";

export interface FileMetadata {
  fileId: string;
  filename: string;
  size: number;
  mimeType?: string;
}

export interface StagedFile {
  id: string;
  name: string;
  size: number;
  path?: string;
}

export interface IncomingTransferPrompt {
  transferId: string;
  senderId: string;
  senderAlias: string;
  senderDeviceModel: string;
  senderIp: string;
  file: FileMetadata;
}

export interface ActiveTransfer {
  transferId: string;
  fileId: string;
  filename: string;
  totalBytes: number;
  bytesTransferred: number;
  speedBytesPerSec: number;
  status: TransferStatus;
  direction: "outgoing" | "incoming";
  peerId: string;
  peerAlias: string;
  errorMessage?: string;
  savedPath?: string;
}

// ==========================================
// 3. Watch Party & Sync Room Types (Section 3.3 & 13)
// ==========================================

export type ParticipantSyncState =
  | "DISCONNECTED"
  | "CONNECTING"
  | "CONNECTED"
  | "WAITING_FOR_MOVIE"
  | "VERIFYING_MOVIE"
  | "LAUNCHING_PLAYER"
  | "LOADING_MOVIE"
  | "WAITING_FOR_SYNC"
  | "SYNCHRONIZING"
  | "WATCHING"
  | "OUT_OF_SYNC"
  | "ERROR";

export interface MovieFileInfo {
  movieId: string;
  filename: string;
  fileSize: number;
  duration: number; // seconds
  containerFormat: string;
  videoCodec?: string;
  audioCodec?: string;
  resolution?: string;
  sha256Hash?: string;
  sha256?: string;
}

export interface MovieVerificationResult {
  status: "VERIFIED" | "MISMATCH" | "NOT_LOADED" | "COMPATIBILITY_APPROVED";
  hashMatched: boolean;
  durationDiff: number;
  compatibilityApproved: boolean;
  reason: string;
  isCompatible?: boolean;
  title?: string;
  details?: string;
  durationMatched?: boolean;
  sizeMatched?: boolean;
}

export interface RoomMember {
  id: string;
  alias: string;
  ip: string;
  isHost: boolean;
  status: ParticipantSyncState;
  movieMatched: boolean;
  driftSeconds: number;
  position?: number;
  pingMs: number;
}

export interface DiscoveredRoom {
  roomId: string;
  title: string;
  hostAlias: string;
  hostIp: string;
  port: number;
  movieTitle: string;
  memberCount: number;
}

export interface RoomControlPermissions {
  allowAll: boolean;
  allowPlayPause: boolean;
  allowJumpForward: boolean;
  allowJumpBackward: boolean;
  allowSeek: boolean;
  allowSpeed: boolean;
}

export interface WatchRoomState {
  roomId: string;
  movieId: string;
  movieInfo?: MovieFileInfo;
  position: number; // seconds
  paused: boolean;
  speed: number;
  timestamp: number; // Unix ms when snapshot was captured
  sequence?: number;
  actor: string;
  strictHostControl: boolean;
  permissions?: RoomControlPermissions;
  isHost?: boolean;
  hostIp?: string;
  port?: number;
  driftSeconds?: number;
  syncStatus?: string;
  movieMatched?: boolean;
}

// ==========================================
// 4. mpv Media Engine Types (Section 2.3 & Phase 4)
// ==========================================

export type MpvEngineStatus =
  | "uninitialized"
  | "starting"
  | "ready"
  | "loading"
  | "playing"
  | "paused"
  | "stopped"
  | "error";

export interface MpvPlaybackStatus {
  engineStatus: MpvEngineStatus;
  timePos: number;
  duration: number;
  pause: boolean;
  speed: number;
  filename?: string;
  bufferPercent?: number;
  errorMessage?: string;
  videoCodec?: string;
  audioCodec?: string;
  resolution?: string;
}

// ==========================================
// 5. App Global State Types
// ==========================================

export type ActiveTab = "wifi-send" | "watch-party";

export interface SystemInfo {
  app_name: string;
  version: string;
  platform: string;
  offline_mode: boolean;
}
