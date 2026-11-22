/**
 * SyncDrop UI Preview Fixtures (Phase 2 Design Verification)
 * NOTE: These are static UI preview fixtures strictly for layout and visual
 * validation as instructed in PROJECT_SPEC.md. Real peer discovery and transfers
 * integrate with the native Rust backend in subsequent phases.
 */

import type {
  PeerDevice,
  StagedFile,
  ActiveTransfer,
  RoomMember,
  MovieFileInfo,
  DiscoveredRoom,
  IncomingTransferPrompt,
} from "./types";

export const IS_DEMO_FIXTURE_MODE = true;

/**
 * Preview peer devices for UI verification matching UI_references/image1.png
 */
export const DEMO_PREVIEW_PEERS: PeerDevice[] = [
  {
    id: "demo-peer-1",
    alias: "Elena-MacBook-Pro",
    deviceModel: "Apple MacBook Pro M3",
    deviceType: "desktop",
    ip: "192.168.1.18",
    port: 53317,
    protocol: "http",
    lastSeen: Date.now(),
    isStale: false,
  },
  {
    id: "demo-peer-2",
    alias: "Marcus-ThinkPad-X1",
    deviceModel: "Lenovo ThinkPad X1 Carbon",
    deviceType: "desktop",
    ip: "192.168.1.88",
    port: 53317,
    protocol: "http",
    lastSeen: Date.now(),
    isStale: false,
  },
  {
    id: "demo-peer-3",
    alias: "Liam-Gaming-Rig",
    deviceModel: "Custom Tower (RTX 4090)",
    deviceType: "desktop",
    ip: "192.168.1.105",
    port: 53317,
    protocol: "http",
    lastSeen: Date.now(),
    isStale: false,
  },
  {
    id: "demo-peer-4",
    alias: "Sophia-Galaxy-S24",
    deviceModel: "Samsung Galaxy S24 Ultra",
    deviceType: "mobile",
    ip: "192.168.1.45",
    port: 53317,
    protocol: "http",
    lastSeen: Date.now(),
    isStale: false,
  },
  {
    id: "demo-peer-5",
    alias: "Dev-ArchLinux-Lab",
    deviceModel: "Linux 6.8 workstation",
    deviceType: "desktop",
    ip: "192.168.1.201",
    port: 53317,
    protocol: "http",
    lastSeen: Date.now(),
    isStale: false,
  },
  {
    id: "demo-peer-6",
    alias: "Chloe-iPad-Air",
    deviceModel: "iPadOS 17.4 Tablet",
    deviceType: "mobile",
    ip: "192.168.1.72",
    port: 53317,
    protocol: "http",
    lastSeen: Date.now() - 10000,
    isStale: true,
  },
];

/**
 * Preview staged files matching UI reference image1.png
 */
export const DEMO_PREVIEW_STAGED_FILES: StagedFile[] = [
  {
    id: "demo-file-1",
    name: "Design_System_Tokens_2026.fig",
    size: 284 * 1024 * 1024, // 284 MB
  },
  {
    id: "demo-file-2",
    name: "Raw_Production_Footage_Take3.mov",
    size: 3.1 * 1024 * 1024 * 1024, // 3.10 GB
  },
  {
    id: "demo-file-3",
    name: "SyncDrop_Client_Windows_v1.4.zip",
    size: 48 * 1024 * 1024, // 48 MB
  },
];

/**
 * Preview active & completed transfers
 */
export const DEMO_PREVIEW_TRANSFERS: ActiveTransfer[] = [
  {
    transferId: "demo-tx-1",
    fileId: "demo-file-2",
    filename: "Raw_Production_Footage_Take3.mov",
    totalBytes: 3.1 * 1024 * 1024 * 1024,
    bytesTransferred: 2.108 * 1024 * 1024 * 1024,
    speedBytesPerSec: 94.2 * 1024 * 1024,
    status: "transferring",
    direction: "outgoing",
    peerId: "demo-peer-1",
    peerAlias: "Elena-MacBook-Pro",
  },
  {
    transferId: "demo-tx-2",
    fileId: "demo-file-1",
    filename: "Design_System_Tokens_2026.fig",
    totalBytes: 284 * 1024 * 1024,
    bytesTransferred: 284 * 1024 * 1024,
    speedBytesPerSec: 112.0 * 1024 * 1024,
    status: "completed",
    direction: "outgoing",
    peerId: "demo-peer-2",
    peerAlias: "Marcus-ThinkPad-X1",
  },
];

export const DEMO_COMPLETED_HISTORY: ActiveTransfer[] = [
  {
    transferId: "demo-hist-1",
    fileId: "demo-hist-f1",
    filename: "Camera_RAW_Batch_042.zip",
    totalBytes: 1.85 * 1024 * 1024 * 1024,
    bytesTransferred: 1.85 * 1024 * 1024 * 1024,
    speedBytesPerSec: 108.5 * 1024 * 1024,
    status: "completed",
    direction: "outgoing",
    peerId: "demo-peer-3",
    peerAlias: "Liam-Gaming-Rig",
  },
  {
    transferId: "demo-hist-2",
    fileId: "demo-hist-f2",
    filename: "Soundtrack_FLAC_Lossless.tar",
    totalBytes: 642 * 1024 * 1024,
    bytesTransferred: 642 * 1024 * 1024,
    speedBytesPerSec: 98.4 * 1024 * 1024,
    status: "completed",
    direction: "incoming",
    peerId: "demo-peer-4",
    peerAlias: "Sophia-Galaxy-S24",
  },
];

/**
 * Preview movie metadata matching UI reference image2.png
 */
export const DEMO_PREVIEW_MOVIE: MovieFileInfo = {
  movieId: "dune-part-two",
  filename: "Dune.Part.Two.2024.1080p.WEBRip.x264.AAC5.1-SyncDrop.mkv",
  fileSize: 14.2 * 1024 * 1024 * 1024, // 14.2 GB
  duration: 9978, // 02:46:18
  containerFormat: "MKV",
  videoCodec: "x264 1080p 60fps",
  audioCodec: "AAC 5.1",
  resolution: "1920x1080",
  sha256: "a8f3d1b942e617d98b251fc43872bb28169992c1",
};

/**
 * Preview connected members matching UI reference image2.png
 */
export const DEMO_PREVIEW_MEMBERS: RoomMember[] = [
  {
    id: "mem-host",
    alias: "Alex-Desktop-PC",
    ip: "192.168.1.42",
    isHost: true,
    status: "WATCHING",
    movieMatched: true,
    driftSeconds: 0.0,
    pingMs: 0.0,
  },
  {
    id: "mem-1",
    alias: "Elena-MacBook-Pro",
    ip: "192.168.1.18",
    isHost: false,
    status: "WATCHING",
    movieMatched: true,
    driftSeconds: 0.004,
    pingMs: 1.2,
  },
  {
    id: "mem-2",
    alias: "Marcus-ThinkPad-X1",
    ip: "192.168.1.88",
    isHost: false,
    status: "WATCHING",
    movieMatched: true,
    driftSeconds: 0.008,
    pingMs: 2.4,
  },
  {
    id: "mem-3",
    alias: "Sophia-Galaxy-S24",
    ip: "192.168.1.45",
    isHost: false,
    status: "SYNCHRONIZING",
    movieMatched: true,
    driftSeconds: 0.042,
    pingMs: 8.1,
  },
  {
    id: "mem-4",
    alias: "Liam-Gaming-Rig",
    ip: "192.168.1.105",
    isHost: false,
    status: "WATCHING",
    movieMatched: true,
    driftSeconds: 0.002,
    pingMs: 0.9,
  },
  {
    id: "mem-5",
    alias: "Chloe-iPad-Air",
    ip: "192.168.1.72",
    isHost: false,
    status: "WATCHING",
    movieMatched: true,
    driftSeconds: 0.012,
    pingMs: 4.5,
  },
  {
    id: "mem-6",
    alias: "Dev-ArchLinux-Lab",
    ip: "192.168.1.201",
    isHost: false,
    status: "WATCHING",
    movieMatched: true,
    driftSeconds: 0.003,
    pingMs: 1.5,
  },
  {
    id: "mem-7",
    alias: "Noah-Pixel-8",
    ip: "192.168.1.91",
    isHost: false,
    status: "WATCHING",
    movieMatched: true,
    driftSeconds: 0.006,
    pingMs: 3.8,
  },
  {
    id: "mem-8",
    alias: "Maya-Yoga-Slim",
    ip: "192.168.1.64",
    isHost: false,
    status: "WAITING_FOR_SYNC",
    movieMatched: true,
    driftSeconds: -3.0,
    pingMs: 6.2,
  },
  {
    id: "mem-9",
    alias: "Kevin-MacMini",
    ip: "192.168.1.112",
    isHost: false,
    status: "ERROR",
    movieMatched: false,
    driftSeconds: 0.0,
    pingMs: 1.8,
  },
];

/**
 * Preview discovered rooms on LAN
 */
export const DEMO_DISCOVERED_ROOMS: DiscoveredRoom[] = [
  {
    roomId: "room-dune",
    title: "Movie Night: Dune Part Two (1080p Atmos Remux)",
    hostAlias: "Alex-Desktop-PC",
    hostIp: "192.168.1.42",
    port: 53318,
    movieTitle: "Dune.Part.Two.2024.1080p.WEBRip.x264.mkv",
    memberCount: 10,
  },
  {
    roomId: "room-interstellar",
    title: "Sci-Fi Weekend: Interstellar IMAX Edition",
    hostAlias: "Marcus-ThinkPad-X1",
    hostIp: "192.168.1.88",
    port: 53318,
    movieTitle: "Interstellar.2014.IMAX.1080p.BluRay.x264.mkv",
    memberCount: 4,
  },
];

/**
 * Sample incoming prompt for modal testing
 */
export const DEMO_SAMPLE_INCOMING_PROMPT: IncomingTransferPrompt = {
  transferId: "incoming-prompt-sample",
  senderId: "demo-peer-1",
  senderAlias: "Elena-MacBook-Pro",
  senderDeviceModel: "Apple MacBook Pro M3",
  senderIp: "192.168.1.18",
  file: {
    fileId: "f-shared-01",
    filename: "Production_Assets_4K_Final.zip",
    size: 2.45 * 1024 * 1024 * 1024,
    mimeType: "application/zip",
  },
};
