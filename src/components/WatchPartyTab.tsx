import React, { useState, useEffect } from "react";
import {
  Film,
  Play,
  Pause,
  RotateCcw,
  RotateCw,
  Copy,
  Check,
  Radio,
  Sliders,
  CheckCircle2,
  Users,
  AlertTriangle,
  FolderOpen,
  FastForward,
  Activity,
  Power,
  Tv,
  Loader2,
  ShieldCheck,
  DownloadCloud,
  Hash,
  Monitor,
  Music,
  Lock,
  Unlock,
  Shield,
} from "lucide-react";
import type {
  RoomMember,
  MovieFileInfo,
  WatchRoomState,
  DiscoveredRoom,
  MpvPlaybackStatus,
  MovieVerificationResult,
  ActiveTransfer,
  RoomControlPermissions,
} from "../types";
import {
  pickMovieFileNative,
  inspectMovieFile,
  getMovieVerification,
  setCompatibilityMode,
} from "../lib/tauriEvents";

interface WatchPartyTabProps {
  roomState: WatchRoomState | null;
  members: RoomMember[];
  selectedMovie: MovieFileInfo | null;
  discoveredRooms: DiscoveredRoom[];
  mpvStatus: MpvPlaybackStatus;
  localIp?: string;
  downloadDir?: string;
  completedTransfers?: ActiveTransfer[];
  onSwitchToWifiSend?: () => void;
  onSelectMovie: (movie: MovieFileInfo) => void;
  onTogglePlay: () => void;
  onSeek: (seconds: number, absolute?: boolean) => void;
  onSpeedChange: (speed: number) => void;
  onLaunchPlayer: (filePath?: string) => void;
  onStopPlayer: () => void;
  onHostRoom: () => void;
  onLeaveRoom?: () => void;
  onJoinRoom: (uri: string) => void;
  onSyncRoom?: () => void;
  onSimulateDrift?: () => void;
  onUpdatePermissions?: (permissions: RoomControlPermissions) => void;
  isDemoMode?: boolean;
}

export const WatchPartyTab: React.FC<WatchPartyTabProps> = ({
  roomState,
  members,
  selectedMovie,
  discoveredRooms,
  mpvStatus,
  localIp,
  downloadDir,
  completedTransfers = [],
  onSwitchToWifiSend,
  onSelectMovie,
  onTogglePlay,
  onSeek,
  onSpeedChange,
  onLaunchPlayer,
  onStopPlayer,
  onHostRoom,
  onLeaveRoom,
  onJoinRoom,
  onSyncRoom,
  onSimulateDrift,
  onUpdatePermissions,
  isDemoMode = false,
}) => {
  const [hostingMode, setHostingMode] = useState(true);
  const [joinUri, setJoinUri] = useState("");
  const [introOffset, setIntroOffset] = useState("+0.00s");
  const [copiedUri, setCopiedUri] = useState(false);
  const [copiedHash, setCopiedHash] = useState(false);
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [verificationResult, setVerificationResult] = useState<MovieVerificationResult | null>(null);
  const [isUpdatingCompatibility, setIsUpdatingCompatibility] = useState(false);

  const defaultPermissions: RoomControlPermissions = {
    allowAll: true,
    allowPlayPause: true,
    allowJumpForward: true,
    allowJumpBackward: true,
    allowSeek: true,
    allowSpeed: true,
  };

  const [permissions, setPermissions] = useState<RoomControlPermissions>(
    roomState?.permissions || defaultPermissions
  );

  useEffect(() => {
    if (roomState?.permissions) {
      setPermissions(roomState.permissions);
    }
  }, [roomState?.permissions]);

  const toggleAllPermissions = (checked: boolean) => {
    const updated: RoomControlPermissions = {
      allowAll: checked,
      allowPlayPause: checked,
      allowJumpForward: checked,
      allowJumpBackward: checked,
      allowSeek: checked,
      allowSpeed: checked,
    };
    setPermissions(updated);
    if (onUpdatePermissions) onUpdatePermissions(updated);
  };

  const toggleSinglePermission = (
    key: "allowPlayPause" | "allowJumpForward" | "allowJumpBackward" | "allowSeek" | "allowSpeed",
    checked: boolean
  ) => {
    const next = { ...permissions, [key]: checked };
    const allOn =
      next.allowPlayPause &&
      next.allowJumpForward &&
      next.allowJumpBackward &&
      next.allowSeek &&
      next.allowSpeed;
    next.allowAll = allOn;
    setPermissions(next);
    if (onUpdatePermissions) onUpdatePermissions(next);
  };

  const isInRoom = Boolean(roomState && roomState.roomId && roomState.roomId.trim().length > 0);
  const isHosting = isInRoom ? Boolean(roomState?.isHost) : hostingMode;
  const currentHostIp = roomState?.hostIp || localIp || "127.0.0.1";
  const currentPort = roomState?.port || 53318;
  const shareableRoomLink = isInRoom && isHosting
    ? `sync://${currentHostIp}:${currentPort}`
    : "Start hosting to generate link";

  const activePermissions = roomState?.permissions || permissions;
  const canPlayPause = isHosting || activePermissions.allowAll || activePermissions.allowPlayPause;
  const canJumpBackward = isHosting || activePermissions.allowAll || activePermissions.allowJumpBackward;
  const canJumpForward = isHosting || activePermissions.allowAll || activePermissions.allowJumpForward;
  const canSeek = isHosting || activePermissions.allowAll || activePermissions.allowSeek;
  const canSpeed = isHosting || activePermissions.allowAll || activePermissions.allowSpeed;

  // Poll or refresh verification status whenever room movie or local movie changes
  useEffect(() => {
    if (isInRoom) {
      getMovieVerification()
        .then((res) => setVerificationResult(res))
        .catch(console.warn);
    } else {
      setVerificationResult(null);
    }
  }, [
    isInRoom,
    roomState?.movieInfo?.filename,
    roomState?.movieInfo?.sha256Hash,
    selectedMovie?.filename,
    selectedMovie?.sha256Hash,
  ]);

  const formatDuration = (totalSeconds: number): string => {
    if (!totalSeconds || isNaN(totalSeconds) || totalSeconds < 0) return "00:00:00";
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = Math.floor(totalSeconds % 60);
    if (hours > 0) {
      return `${hours.toString().padStart(2, "0")}:${minutes
        .toString()
        .padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;
    }
    return `${minutes.toString().padStart(2, "0")}:${seconds
      .toString()
      .padStart(2, "0")}`;
  };

  const formatBytes = (bytes: number): string => {
    if (!bytes || bytes <= 0) return "0 MB";
    const mb = bytes / (1024 * 1024);
    if (mb >= 1024) {
      return `${(mb / 1024).toFixed(2)} GB`;
    }
    return `${mb.toFixed(1)} MB`;
  };

  const handleCopyUri = () => {
    navigator.clipboard.writeText(shareableRoomLink);
    setCopiedUri(true);
    setTimeout(() => setCopiedUri(false), 2000);
  };

  const handleCopyHash = (hash: string) => {
    navigator.clipboard.writeText(hash);
    setCopiedHash(true);
    setTimeout(() => setCopiedHash(false), 2000);
  };

  const loadAndVerifyMovie = async (filePath: string) => {
    setIsAnalyzing(true);
    try {
      // 1. Asynchronously read metadata & calculate SHA-256 via worker pool
      const inspected = await inspectMovieFile(filePath);
      inspected.filePath = filePath;
      await onSelectMovie(inspected);

      // 2. Query backend verification evaluator
      const res = await getMovieVerification();
      setVerificationResult(res);

      // 3. For host or standalone player mode, launch player with file
      if (!isInRoom || roomState?.isHost) {
        await onLaunchPlayer(filePath);
      }
      // Note: for room participants, setLocalWatchMovie (dispatched by onSelectMovie)
      // automatically loads the file into mpv in PAUSED state and catches up to the host!
    } catch (err) {
      console.error("Failed to inspect and load movie:", err);
    } finally {
      setIsAnalyzing(false);
    }
  };

  const handleBrowseMovieClick = async () => {
    const selectedPath = await pickMovieFileNative();
    if (selectedPath) {
      await loadAndVerifyMovie(selectedPath);
    }
  };

  const handleToggleCompatibility = async (approve: boolean) => {
    setIsUpdatingCompatibility(true);
    try {
      const res = await setCompatibilityMode(approve);
      if (res) {
        setVerificationResult(res);
      }
      if (approve && onSyncRoom && isInRoom && !roomState?.isHost) {
        setTimeout(() => {
          onSyncRoom();
        }, 400);
      }
    } catch (err) {
      console.warn("Failed to set compatibility mode:", err);
    } finally {
      setIsUpdatingCompatibility(false);
    }
  };

  // Find if any recently received incoming transfer matches the room's movie
  const hostMovie = roomState?.movieInfo;
  const matchingTransfer = completedTransfers.find((t) => {
    if (t.direction !== "incoming" || t.status !== "completed") return false;
    if (!hostMovie) return false;
    // Check filename match or close size match
    const filenameMatch = t.filename.toLowerCase() === hostMovie.filename.toLowerCase();
    const sizeMatch = Math.abs(t.totalBytes - hostMovie.fileSize) < 1024 * 1024;
    return filenameMatch || sizeMatch;
  });

  const handleLoadReceivedMovie = async () => {
    if (!matchingTransfer) return;
    const targetPath =
      matchingTransfer.savedPath ||
      (downloadDir ? `${downloadDir}\\${matchingTransfer.filename}` : matchingTransfer.filename);
    await loadAndVerifyMovie(targetPath);
  };

  const isPlayerRunning =
    mpvStatus.engineStatus === "ready" ||
    mpvStatus.engineStatus === "playing" ||
    mpvStatus.engineStatus === "paused" ||
    mpvStatus.engineStatus === "loading";

  const isPlaying = !mpvStatus.pause && isPlayerRunning;
  const currentPos = mpvStatus.timePos > 0 ? mpvStatus.timePos : 0;
  const totalDuration =
    mpvStatus.duration > 0
      ? mpvStatus.duration
      : selectedMovie?.duration || roomState?.movieInfo?.duration || 7200;
  const progressPercent = Math.min(100, Math.max(0, (currentPos / (totalDuration || 1)) * 100));

  const syncedCount = members.filter(
    (m) => m.movieMatched && Math.abs(m.driftSeconds) < 0.3
  ).length;

  const currentSha256 = selectedMovie?.sha256Hash;
  const currentResolution =
    mpvStatus.resolution || selectedMovie?.resolution || "1080p";
  const currentVideoCodec =
    mpvStatus.videoCodec || selectedMovie?.videoCodec || "H.264";
  const currentAudioCodec =
    mpvStatus.audioCodec || selectedMovie?.audioCodec || "AAC";

  return (
    <div className="flex-1 flex flex-col overflow-hidden bg-slate-50/60 select-none">
      {/* Top Banner Inside Tab */}
      <div className="px-6 py-2 bg-blue-50/60 border-b border-blue-100/80 flex items-center justify-between text-xs text-blue-950 flex-wrap gap-2">
        <div className="flex items-center gap-3">
          <span className="font-mono bg-white border border-blue-200/90 px-2 py-0.5 rounded-md text-[11px] font-semibold text-slate-800 shadow-2xs">
            {isInRoom
              ? roomState?.isHost
                ? `Hosting Room (${currentHostIp}:${currentPort})`
                : `Connected to Room (${currentHostIp}:${currentPort})`
              : "No Active Room"}
          </span>
          {isInRoom && (
            <span className="flex items-center gap-1.5 font-semibold text-emerald-800 bg-emerald-50 border border-emerald-200 px-2.5 py-0.5 rounded-md text-[11px]">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse" />
              <span>{members.length} friends in room</span>
            </span>
          )}
          <span className="hidden md:flex items-center gap-1 text-[11px] text-slate-600 font-mono">
            <Activity className="w-3 h-3 text-blue-600" />
            <span>Local LAN PTP Sync: &lt;1.8ms jitter</span>
          </span>
          {isDemoMode && (
            <span className="text-[10px] font-medium text-amber-800 bg-amber-50 border border-amber-200 px-1.5 py-0.2 rounded">
              Demo Fixtures
            </span>
          )}
        </div>
        <div className="flex items-center gap-3 text-[11px]">
          {isInRoom && !roomState?.isHost && onSyncRoom && (
            <button
              onClick={onSyncRoom}
              className="flex items-center gap-1 text-blue-700 hover:text-blue-800 bg-blue-50 hover:bg-blue-100/80 border border-blue-200 px-2.5 py-1 rounded-lg font-semibold transition active:scale-95 shadow-2xs"
              title="Request fresh authoritative playback state snapshot from host"
            >
              <RotateCcw className="w-3.5 h-3.5 text-blue-600" />
              <span>Resync</span>
            </button>
          )}
          {isInRoom && onLeaveRoom && (
            <button
              onClick={onLeaveRoom}
              className="flex items-center gap-1 text-rose-700 hover:text-rose-800 bg-rose-50 hover:bg-rose-100/80 border border-rose-200 px-2.5 py-1 rounded-lg font-semibold transition active:scale-95 shadow-2xs"
            >
              <Power className="w-3.5 h-3.5 text-rose-600" />
              <span>Leave Room</span>
            </button>
          )}
          {onSimulateDrift && (
            <button
              onClick={onSimulateDrift}
              className="flex items-center gap-1 text-amber-800 hover:text-amber-900 bg-amber-50 hover:bg-amber-100/80 border border-amber-200 px-2.5 py-1 rounded-lg font-semibold transition active:scale-95 shadow-2xs"
            >
              <AlertTriangle className="w-3.5 h-3.5 text-amber-600" />
              <span>Simulate Drift Event</span>
            </button>
          )}
          <span className="font-mono text-slate-500">WebSocket: {currentPort}</span>
        </div>
      </div>

      {/* Main Grid: Left cards (Col 7) and Right cards (Col 5) */}
      <div className="flex-1 grid grid-cols-12 gap-6 p-6 overflow-y-auto">
        {/* ============================================================== */}
        {/* Left Column: Room Controls, Movie, Scrubber (Col 7)            */}
        {/* ============================================================== */}
        <div className="col-span-12 lg:col-span-7 flex flex-col gap-5">
          {/* Card 1: Room Connection Mode */}
          <div className="bg-white rounded-2xl border border-slate-200 p-5 shadow-2xs flex flex-col gap-4">
            <div className="flex items-center justify-between flex-wrap gap-2">
              <div className="flex items-center gap-2">
                <Radio className="w-4 h-4 text-blue-600" />
                <h3 className="text-xs font-bold uppercase tracking-wider text-slate-900">
                  Room Connection Mode
                </h3>
              </div>
              <div className="flex items-center bg-slate-100 p-0.5 rounded-xl border border-slate-200 text-xs font-semibold">
                <button
                  onClick={() => setHostingMode(true)}
                  className={`px-3.5 py-1.5 rounded-lg transition-all ${
                    isHosting
                      ? "bg-white text-blue-700 shadow-xs border border-slate-200/50"
                      : "text-slate-500 hover:text-slate-900"
                  }`}
                >
                  Host a Room
                </button>
                <button
                  onClick={() => setHostingMode(false)}
                  className={`px-3.5 py-1.5 rounded-lg transition-all ${
                    !isHosting
                      ? "bg-white text-blue-700 shadow-xs border border-slate-200/50"
                      : "text-slate-500 hover:text-slate-900"
                  }`}
                >
                  Join a Room
                </button>
              </div>
            </div>

            {isHosting ? (
              <div className="flex flex-col gap-3">
                <div className="flex items-center justify-between p-3 bg-slate-50 border border-slate-200/90 rounded-xl text-xs flex-wrap gap-2">
                  <div className="flex items-center gap-2">
                    <span className="text-slate-500">Your Shareable Room Link:</span>
                    <span className="font-mono font-bold text-slate-800 bg-white px-2 py-0.5 rounded border border-slate-200">
                      {shareableRoomLink}
                    </span>
                  </div>
                  <button
                    onClick={handleCopyUri}
                    disabled={!isInRoom || !isHosting}
                    className="flex items-center gap-1 bg-white hover:bg-slate-100 disabled:opacity-50 disabled:cursor-not-allowed border border-slate-200 text-slate-700 px-2.5 py-1 rounded-lg font-semibold transition active:scale-95 shadow-2xs"
                  >
                    {copiedUri ? (
                      <>
                        <Check className="w-3.5 h-3.5 text-emerald-600" />
                        <span className="text-emerald-700">Copied</span>
                      </>
                    ) : (
                      <>
                        <Copy className="w-3.5 h-3.5 text-slate-500" />
                        <span>Copy Link</span>
                      </>
                    )}
                  </button>
                </div>

                <div className="flex items-center justify-between pt-1 flex-wrap gap-2 text-xs">
                  <div className="flex items-center gap-2">
                    <span
                      className={`w-2 h-2 rounded-full ${
                        isInRoom && isHosting ? "bg-emerald-500 animate-pulse" : "bg-slate-300"
                      }`}
                    />
                    <span className="font-semibold text-slate-800">
                      Room Status: {isInRoom && isHosting ? "Active & Broadcasting on LAN" : "Ready to Host"}
                    </span>
                  </div>
                  {(!isInRoom || !isHosting) && (
                    <button
                      onClick={onHostRoom}
                      className="px-3 py-1 bg-blue-600 hover:bg-blue-700 text-white rounded-lg text-xs font-semibold shadow-2xs transition active:scale-95"
                    >
                      Start Hosting Now
                    </button>
                  )}
                </div>

                {/* Granular Participant Permissions Control Panel */}
                <div className="bg-slate-50 border border-slate-200/90 rounded-xl p-3.5 flex flex-col gap-2.5">
                  <div className="flex items-center justify-between flex-wrap gap-2">
                    <div className="flex items-center gap-1.5">
                      <Shield className="w-3.5 h-3.5 text-blue-600" />
                      <span className="text-[11px] font-bold uppercase tracking-wider text-slate-800">
                        Participant Control Permissions
                      </span>
                      <span className="text-[10px] text-slate-500 hidden sm:inline">
                        (Choose which controls connected friends can use)
                      </span>
                    </div>

                    {/* Master ALL toggle */}
                    <label className="flex items-center gap-1.5 px-2.5 py-1 bg-white border border-slate-200 hover:border-blue-300 rounded-lg cursor-pointer transition text-[11px] font-semibold text-slate-800 shadow-2xs">
                      <input
                        type="checkbox"
                        checked={permissions.allowAll}
                        onChange={(e) => toggleAllPermissions(e.target.checked)}
                        className="rounded text-blue-600 focus:ring-blue-500 w-3.5 h-3.5"
                      />
                      <span>All Controls (Full Access)</span>
                    </label>
                  </div>

                  {/* Individual Control Checkboxes */}
                  <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-2.5 pt-2 border-t border-slate-200/70 text-xs">
                    <label className="flex items-center gap-1.5 text-slate-700 hover:text-slate-900 cursor-pointer select-none">
                      <input
                        type="checkbox"
                        checked={permissions.allowPlayPause}
                        onChange={(e) => toggleSinglePermission("allowPlayPause", e.target.checked)}
                        className="rounded text-blue-600 focus:ring-blue-500 w-3.5 h-3.5"
                      />
                      <span className="text-[11px] font-medium">Play / Pause</span>
                    </label>

                    <label className="flex items-center gap-1.5 text-slate-700 hover:text-slate-900 cursor-pointer select-none">
                      <input
                        type="checkbox"
                        checked={permissions.allowJumpBackward}
                        onChange={(e) => toggleSinglePermission("allowJumpBackward", e.target.checked)}
                        className="rounded text-blue-600 focus:ring-blue-500 w-3.5 h-3.5"
                      />
                      <span className="text-[11px] font-medium">Rewind (-10s)</span>
                    </label>

                    <label className="flex items-center gap-1.5 text-slate-700 hover:text-slate-900 cursor-pointer select-none">
                      <input
                        type="checkbox"
                        checked={permissions.allowJumpForward}
                        onChange={(e) => toggleSinglePermission("allowJumpForward", e.target.checked)}
                        className="rounded text-blue-600 focus:ring-blue-500 w-3.5 h-3.5"
                      />
                      <span className="text-[11px] font-medium">Forward (+10s)</span>
                    </label>

                    <label className="flex items-center gap-1.5 text-slate-700 hover:text-slate-900 cursor-pointer select-none">
                      <input
                        type="checkbox"
                        checked={permissions.allowSeek}
                        onChange={(e) => toggleSinglePermission("allowSeek", e.target.checked)}
                        className="rounded text-blue-600 focus:ring-blue-500 w-3.5 h-3.5"
                      />
                      <span className="text-[11px] font-medium">Timeline Seek</span>
                    </label>

                    <label className="flex items-center gap-1.5 text-slate-700 hover:text-slate-900 cursor-pointer select-none">
                      <input
                        type="checkbox"
                        checked={permissions.allowSpeed}
                        onChange={(e) => toggleSinglePermission("allowSpeed", e.target.checked)}
                        className="rounded text-blue-600 focus:ring-blue-500 w-3.5 h-3.5"
                      />
                      <span className="text-[11px] font-medium">Speed Control</span>
                    </label>
                  </div>
                </div>
              </div>
            ) : (
              <div className="flex flex-col gap-4">
                <div className="flex items-center gap-2">
                  <input
                    type="text"
                    placeholder="Enter Host IP or sync://192.168.1.x:53318"
                    value={joinUri}
                    onChange={(e) => setJoinUri(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && joinUri.trim()) {
                        onJoinRoom(joinUri.trim());
                      }
                    }}
                    className="flex-1 bg-white border border-slate-200 rounded-xl px-3.5 py-2 text-xs text-slate-800 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 transition"
                  />
                  <button
                    onClick={() => {
                      if (joinUri.trim()) {
                        onJoinRoom(joinUri.trim());
                      }
                    }}
                    disabled={!joinUri.trim()}
                    className="px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-xl text-xs font-semibold shadow-2xs transition active:scale-95 flex-shrink-0"
                  >
                    Join Room
                  </button>
                </div>

                {/* Discovered Rooms List */}
                <div className="flex flex-col gap-2">
                  <div className="flex items-center justify-between">
                    <span className="text-[11px] font-bold text-slate-500 uppercase tracking-wider">
                      Discovered Rooms on LAN
                    </span>
                    <span className="text-[11px] text-slate-400 font-mono">
                      {discoveredRooms.length} rooms found
                    </span>
                  </div>

                  <div className="flex flex-col gap-2 max-h-48 overflow-y-auto">
                    {discoveredRooms.length === 0 ? (
                      <div className="p-4 bg-slate-50 border border-slate-200/80 rounded-xl text-center text-xs text-slate-400">
                        Scanning Wi-Fi subnet for watch rooms... Or enter a host IP above.
                      </div>
                    ) : (
                      discoveredRooms.map((room) => (
                        <div
                          key={room.roomId}
                          className="p-3 bg-slate-50 border border-slate-200 rounded-xl flex items-center justify-between text-xs hover:border-blue-300 transition"
                        >
                          <div className="min-w-0 pr-2">
                            <p className="font-semibold text-slate-900 truncate">{room.title}</p>
                            <p className="text-[11px] text-slate-500 truncate mt-0.5">
                              Host: <span className="font-medium text-slate-700">{room.hostAlias}</span> (
                              <span className="font-mono">{room.hostIp}</span>) • {room.memberCount} members
                            </p>
                          </div>
                          <button
                            onClick={() => onJoinRoom(`sync://${room.hostIp}:${room.port}`)}
                            className="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded-lg text-xs font-semibold shadow-2xs flex-shrink-0 transition"
                          >
                            Join
                          </button>
                        </div>
                      ))
                    )}
                  </div>
                </div>
              </div>
            )}
          </div>

          {/* Card 2: Local Movie File & mpv Sidecar Status */}
          <div className="bg-white rounded-2xl border border-slate-200 p-5 shadow-2xs flex flex-col gap-4">
            <div className="flex items-center justify-between flex-wrap gap-2">
              <div>
                <div className="flex items-center gap-2">
                  <Film className="w-4 h-4 text-blue-600" />
                  <h3 className="text-xs font-bold uppercase tracking-wider text-slate-900">
                    Local Movie File &amp; mpv Engine
                  </h3>
                </div>
                <p className="text-xs text-slate-500 mt-0.5">
                  Controlled directly via Windows Named Pipe JSON IPC without external players.
                </p>
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={handleBrowseMovieClick}
                  className="flex items-center gap-1.5 px-3 py-1.5 bg-slate-100 hover:bg-slate-200 text-slate-700 rounded-xl text-xs font-semibold transition active:scale-95 shadow-2xs"
                >
                  <FolderOpen className="w-3.5 h-3.5 text-blue-600" />
                  <span>Select Video File</span>
                </button>
                {isPlayerRunning ? (
                  <button
                    onClick={onStopPlayer}
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-red-50 hover:bg-red-100 text-red-700 border border-red-200 rounded-xl text-xs font-semibold transition active:scale-95"
                    title="Terminate mpv player process"
                  >
                    <Power className="w-3.5 h-3.5 text-red-600" />
                    <span>Close mpv</span>
                  </button>
                ) : (
                  <button
                    onClick={() => onLaunchPlayer()}
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-50 hover:bg-blue-100 text-blue-700 border border-blue-200 rounded-xl text-xs font-semibold transition active:scale-95"
                    title="Launch mpv player sidecar"
                  >
                    <Tv className="w-3.5 h-3.5 text-blue-600" />
                    <span>Launch Player</span>
                  </button>
                )}
              </div>
            </div>

            {/* Asynchronous Hashing / Media Inspection Progress */}
            {isAnalyzing && (
              <div className="bg-blue-50 border border-blue-200 rounded-xl p-3 flex items-center gap-3 text-xs text-blue-900">
                <Loader2 className="w-4 h-4 text-blue-600 animate-spin flex-shrink-0" />
                <div className="flex-1">
                  <p className="font-semibold">Inspecting Media &amp; Computing SHA-256 Hash...</p>
                  <p className="text-[11px] text-blue-700">
                    Running in background worker pool to prevent freezing the UI.
                  </p>
                </div>
              </div>
            )}

            {/* Missing Movie & Wi-Fi Send Integration Workflow */}
            {isInRoom && hostMovie && !roomState.isHost && (
              <>
                {/* 1. If an incoming Wi-Fi Send transfer just completed for this movie */}
                {matchingTransfer && (
                  <div className="bg-emerald-50 border border-emerald-300 rounded-xl p-3.5 flex items-center justify-between text-xs text-emerald-900 gap-3 shadow-2xs">
                    <div className="flex items-center gap-2.5 min-w-0">
                      <div className="w-8 h-8 rounded-lg bg-emerald-600 text-white flex items-center justify-center flex-shrink-0">
                        <DownloadCloud className="w-4 h-4" />
                      </div>
                      <div className="min-w-0">
                        <p className="font-bold truncate">
                          Movie Received via Wi-Fi Send: {matchingTransfer.filename}
                        </p>
                        <p className="text-[11px] text-emerald-700">
                          {formatBytes(matchingTransfer.totalBytes)} • Ready to verify and auto-synchronize with host.
                        </p>
                      </div>
                    </div>
                    <button
                      onClick={handleLoadReceivedMovie}
                      className="px-3.5 py-1.5 bg-emerald-600 hover:bg-emerald-700 text-white rounded-lg font-semibold flex items-center gap-1.5 flex-shrink-0 transition active:scale-95 shadow-2xs"
                    >
                      <Play className="w-3.5 h-3.5 fill-white" />
                      <span>Load &amp; Catch Up</span>
                    </button>
                  </div>
                )}

                {/* 2. If movie is not loaded yet */}
                {(!selectedMovie || verificationResult?.status === "NOT_LOADED") && !matchingTransfer && (
                  <div className="bg-amber-50 border border-amber-200 rounded-xl p-3.5 flex items-start gap-3 text-xs text-amber-900">
                    <AlertTriangle className="w-5 h-5 text-amber-600 flex-shrink-0 mt-0.5" />
                    <div className="flex-1 min-w-0">
                      <p className="font-bold">Missing Movie Notice (Section 3.3 &amp; 13.9)</p>
                      <p className="text-[11px] text-amber-800 mt-0.5">
                        Room host is playing <span className="font-semibold underline">{hostMovie.filename}</span> ({formatBytes(hostMovie.fileSize)}).
                        To join synchronized playback, load your local copy or ask the host to send it over high-speed Wi-Fi Send.
                      </p>
                    </div>
                    <div className="flex items-center gap-2 flex-shrink-0">
                      {onSwitchToWifiSend && (
                        <button
                          onClick={onSwitchToWifiSend}
                          className="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded-lg text-xs font-semibold shadow-2xs transition flex items-center gap-1"
                          title="Switch to Wi-Fi Send to receive the file from host"
                        >
                          <DownloadCloud className="w-3.5 h-3.5" />
                          <span>Wi-Fi Send</span>
                        </button>
                      )}
                      <button
                        onClick={handleBrowseMovieClick}
                        className="px-3 py-1.5 bg-amber-600 hover:bg-amber-700 text-white rounded-lg text-xs font-semibold shadow-2xs transition"
                      >
                        Browse File
                      </button>
                    </div>
                  </div>
                )}

                {/* 3. If movie is loaded but verification failed (MISMATCH) */}
                {selectedMovie && verificationResult?.status === "MISMATCH" && (
                  <div className="bg-rose-50 border border-rose-200 rounded-xl p-3.5 flex items-start gap-3 text-xs text-rose-900">
                    <AlertTriangle className="w-5 h-5 text-rose-600 flex-shrink-0 mt-0.5" />
                    <div className="flex-1 min-w-0">
                      <p className="font-bold">Movie Verification Mismatch</p>
                      <p className="text-[11px] text-rose-800 mt-0.5">
                        {verificationResult.reason || "Local file does not match the room host's movie."}
                      </p>
                      <p className="text-[10px] text-rose-700 mt-1">
                        Different encodings or cuts may cause playback desync. You can explicitly approve compatibility mode if you wish to proceed.
                      </p>
                    </div>
                    <div className="flex items-center gap-2 flex-shrink-0">
                      <button
                        onClick={() => handleToggleCompatibility(true)}
                        disabled={isUpdatingCompatibility}
                        className="px-3 py-1.5 bg-rose-600 hover:bg-rose-700 disabled:opacity-50 text-white rounded-lg text-xs font-semibold shadow-2xs transition flex items-center gap-1.5"
                      >
                        <Sliders className="w-3.5 h-3.5" />
                        <span>Approve Compatibility</span>
                      </button>
                    </div>
                  </div>
                )}

                {/* 4. If compatibility mode was approved */}
                {selectedMovie && verificationResult?.status === "COMPATIBILITY_APPROVED" && (
                  <div className="bg-purple-50 border border-purple-200 rounded-xl p-3 flex items-center justify-between text-xs text-purple-900 gap-3">
                    <div className="flex items-center gap-2">
                      <Sliders className="w-4 h-4 text-purple-600 flex-shrink-0" />
                      <div>
                        <p className="font-semibold">Compatibility Mode Active</p>
                        <p className="text-[11px] text-purple-700">
                          Cross-encoding playback explicitly approved by user.
                        </p>
                      </div>
                    </div>
                    <button
                      onClick={() => handleToggleCompatibility(false)}
                      disabled={isUpdatingCompatibility}
                      className="px-2.5 py-1 bg-white hover:bg-purple-100 border border-purple-300 text-purple-800 rounded-md text-[11px] font-semibold transition"
                    >
                      Revoke
                    </button>
                  </div>
                )}
              </>
            )}

            {/* Movie Details Box */}
            <div className="bg-slate-50 border border-slate-200 rounded-xl p-4 flex flex-col gap-3">
              <div className="flex items-center justify-between flex-wrap gap-3">
                <div className="flex items-center gap-3.5 min-w-0">
                  <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-blue-600 to-indigo-600 text-white flex items-center justify-center flex-shrink-0 shadow-md shadow-blue-500/20">
                    <Play className="w-5 h-5 fill-white ml-0.5" />
                  </div>
                  <div className="min-w-0">
                    <p className="font-semibold text-xs text-slate-900 truncate">
                      {mpvStatus.filename || (selectedMovie ? selectedMovie.filename : "No movie file loaded")}
                    </p>
                    <div className="flex items-center gap-2 mt-1 text-[11px] text-slate-500 flex-wrap">
                      <span className="font-mono bg-white px-2 py-0.5 rounded border border-slate-200 font-semibold text-slate-700">
                        {formatDuration(totalDuration)}
                      </span>
                      <span>•</span>
                      <span className="text-slate-600 font-medium">
                        Status: <span className="text-blue-700 capitalize font-bold">{mpvStatus.engineStatus}</span>
                      </span>
                      <span>•</span>
                      <span className="text-slate-600 font-mono">
                        Speed: {mpvStatus.speed.toFixed(2)}x
                      </span>
                    </div>
                  </div>
                </div>

                {/* Match Badge */}
                <div
                  className={`border rounded-xl px-3.5 py-2 text-center flex-shrink-0 ${
                    !isInRoom
                      ? "bg-slate-100 border-slate-200 text-slate-700"
                      : roomState?.isHost
                      ? "bg-blue-50 border-blue-200 text-blue-900"
                      : verificationResult?.status === "VERIFIED" || roomState?.movieMatched
                      ? "bg-emerald-50 border-emerald-200 text-emerald-900"
                      : verificationResult?.status === "COMPATIBILITY_APPROVED"
                      ? "bg-purple-50 border-purple-200 text-purple-900"
                      : verificationResult?.status === "NOT_LOADED"
                      ? "bg-slate-100 border-slate-300 text-slate-600"
                      : "bg-amber-50 border-amber-200 text-amber-900"
                  }`}
                >
                  <div className="flex items-center justify-center gap-1.5 text-xs font-bold">
                    {!isInRoom ? (
                      <>
                        <Film className="w-4 h-4 text-slate-500" />
                        <span>LOCAL PLAYER</span>
                      </>
                    ) : roomState?.isHost ? (
                      <>
                        <ShieldCheck className="w-4 h-4 text-blue-600" />
                        <span>HOST AUTHORITATIVE ({syncedCount}/{members.length})</span>
                      </>
                    ) : verificationResult?.status === "VERIFIED" || roomState?.movieMatched ? (
                      <>
                        <CheckCircle2 className="w-4 h-4 text-emerald-600" />
                        <span>SHA-256 VERIFIED</span>
                      </>
                    ) : verificationResult?.status === "COMPATIBILITY_APPROVED" ? (
                      <>
                        <Sliders className="w-4 h-4 text-purple-600" />
                        <span>COMPATIBILITY MODE</span>
                      </>
                    ) : verificationResult?.status === "NOT_LOADED" ? (
                      <>
                        <Film className="w-4 h-4 text-slate-500" />
                        <span>NOT LOADED</span>
                      </>
                    ) : (
                      <>
                        <AlertTriangle className="w-4 h-4 text-amber-600" />
                        <span>FILE MISMATCH</span>
                      </>
                    )}
                  </div>
                  <span className="text-[10px] opacity-80 font-medium block mt-0.5">
                    {!isInRoom
                      ? "Independent playback"
                      : roomState?.isHost
                      ? "Broadcasting state to peers"
                      : verificationResult?.status === "VERIFIED" || roomState?.movieMatched
                      ? "Identical file & duration"
                      : verificationResult?.status === "COMPATIBILITY_APPROVED"
                      ? "Approved cross-encoding"
                      : verificationResult?.status === "NOT_LOADED"
                      ? "Waiting for movie file"
                      : "Hash or length difference"}
                  </span>
                </div>
              </div>

              {/* Extended Metadata: Codecs, Resolution, and SHA-256 Fingerprint */}
              {selectedMovie && (
                <div className="pt-2 border-t border-slate-200/80 flex items-center justify-between text-[11px] text-slate-600 flex-wrap gap-2">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="flex items-center gap-1 bg-white px-2 py-0.5 rounded border border-slate-200 font-mono text-[10px]">
                      <Monitor className="w-3 h-3 text-slate-400" />
                      <span>{currentResolution}</span>
                    </span>
                    <span className="flex items-center gap-1 bg-white px-2 py-0.5 rounded border border-slate-200 font-mono text-[10px]">
                      <Film className="w-3 h-3 text-slate-400" />
                      <span>{currentVideoCodec}</span>
                    </span>
                    <span className="flex items-center gap-1 bg-white px-2 py-0.5 rounded border border-slate-200 font-mono text-[10px]">
                      <Music className="w-3 h-3 text-slate-400" />
                      <span>{currentAudioCodec}</span>
                    </span>
                    <span className="bg-white px-2 py-0.5 rounded border border-slate-200 font-mono text-[10px]">
                      {formatBytes(selectedMovie.fileSize)}
                    </span>
                  </div>

                  {currentSha256 && (
                    <div
                      onClick={() => handleCopyHash(currentSha256)}
                      className="flex items-center gap-1 bg-white hover:bg-slate-100 border border-slate-200 px-2 py-0.5 rounded cursor-pointer transition font-mono text-[10px] text-slate-500"
                      title="Click to copy full SHA-256 fingerprint"
                    >
                      <Hash className="w-3 h-3 text-blue-500" />
                      <span>
                        SHA: {currentSha256.slice(0, 8)}...{currentSha256.slice(-8)}
                      </span>
                      {copiedHash ? (
                        <Check className="w-3 h-3 text-emerald-600 ml-0.5" />
                      ) : (
                        <Copy className="w-3 h-3 text-slate-400 ml-0.5" />
                      )}
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Intro Offset Adjuster */}
            <div className="flex items-center justify-between bg-slate-50 p-3 rounded-xl border border-slate-200 text-xs flex-wrap gap-2">
              <div>
                <span className="font-semibold text-slate-800">Audio/Video Intro Offset:</span>
                <span className="ml-1.5 font-mono text-blue-600 font-bold">{introOffset}</span>
                <p className="text-[10px] text-slate-500 mt-0.5">
                  Compensates for intro logos or custom web trims (Range: -5.0s to +15.0s)
                </p>
              </div>
              <div className="flex items-center gap-1 font-mono text-xs">
                {["-0.5s", "-0.1s", "0.0s", "+0.1s", "+0.5s"].map((val) => (
                  <button
                    key={val}
                    onClick={() => setIntroOffset(val)}
                    className={`px-2.5 py-1 border rounded-md text-[11px] font-semibold transition ${
                      introOffset === val
                        ? "bg-blue-600 text-white border-blue-600 shadow-2xs"
                        : "bg-white hover:bg-slate-100 border-slate-200 text-slate-700"
                    }`}
                  >
                    {val}
                  </button>
                ))}
              </div>
            </div>
          </div>

          {/* Card 3: Ready Check & Playback Sync Engine */}
          <div className="bg-white rounded-2xl border border-slate-200 p-5 shadow-2xs flex flex-col gap-4">
            <div className="flex items-center justify-between flex-wrap gap-2">
              <div className="flex items-center gap-2 flex-wrap">
                <Sliders className="w-4 h-4 text-blue-600" />
                <h3 className="text-xs font-bold uppercase tracking-wider text-slate-900">
                  Interactive Playback Controls
                </h3>
                {isInRoom && !isHosting && (
                  <span
                    className={`text-[10px] font-semibold px-2 py-0.5 rounded-md border flex items-center gap-1 ${
                      canPlayPause && canSeek && canSpeed && canJumpForward && canJumpBackward
                        ? "bg-emerald-50 text-emerald-800 border-emerald-200"
                        : canPlayPause || canSeek || canJumpForward || canJumpBackward || canSpeed
                        ? "bg-blue-50 text-blue-800 border-blue-200"
                        : "bg-amber-50 text-amber-900 border-amber-200"
                    }`}
                  >
                    {canPlayPause && canSeek && canSpeed && canJumpForward && canJumpBackward ? (
                      <>
                        <Unlock className="w-3 h-3 text-emerald-600" />
                        <span>Host Granted: All Controls</span>
                      </>
                    ) : canPlayPause || canSeek || canJumpForward || canJumpBackward || canSpeed ? (
                      <>
                        <Shield className="w-3 h-3 text-blue-600" />
                        <span>
                          Allowed:{" "}
                          {[
                            canPlayPause && "Play/Pause",
                            (canJumpBackward || canJumpForward) && "Skip",
                            canSeek && "Seek",
                            canSpeed && "Speed",
                          ]
                            .filter(Boolean)
                            .join(", ")}
                        </span>
                      </>
                    ) : (
                      <>
                        <Lock className="w-3 h-3 text-amber-600" />
                        <span>Controls Restricted by Host</span>
                      </>
                    )}
                  </span>
                )}
              </div>
              <div className="flex items-center gap-1.5 text-xs text-slate-600">
                <span
                  className={`w-2 h-2 rounded-full ${
                    isPlayerRunning ? "bg-emerald-500 animate-pulse" : "bg-slate-400"
                  }`}
                />
                <span className="font-medium">mpv Sidecar:</span>
                <span
                  className={`font-bold capitalize ${
                    isPlayerRunning ? "text-emerald-700" : "text-slate-500"
                  }`}
                >
                  {mpvStatus.engineStatus}
                </span>
              </div>
            </div>

            {/* Embedded Player Scrubber Control Bar */}
            <div className="bg-slate-900 rounded-2xl p-4 text-white flex flex-col gap-3 shadow-lg">
              <div className="flex items-center justify-between text-xs text-slate-300">
                <div className="flex items-center gap-2 truncate">
                  <span className="font-mono text-emerald-400 font-bold">
                    {formatDuration(currentPos)}
                  </span>
                  <span>•</span>
                  <span className="text-slate-300 font-medium truncate">
                    {mpvStatus.filename || selectedMovie?.filename || "SyncDrop Movie"}
                  </span>
                </div>
                <span className="font-mono text-slate-400">{formatDuration(totalDuration)}</span>
              </div>

              {/* Progress Slider (Interactive Seeking) */}
              <div
                onClick={(e) => {
                  if (!canSeek) return;
                  const rect = e.currentTarget.getBoundingClientRect();
                  const clickX = e.clientX - rect.left;
                  const ratio = Math.max(0, Math.min(1, clickX / rect.width));
                  const newSec = Math.round(ratio * totalDuration);
                  onSeek(newSec, true);
                }}
                className={`relative w-full h-2.5 rounded-full overflow-hidden transition-all ${
                  canSeek
                    ? "bg-slate-700 hover:h-3 cursor-pointer"
                    : "bg-slate-800 cursor-not-allowed opacity-75"
                }`}
                title={canSeek ? "Click to seek" : "Timeline seek restricted by host"}
              >
                <div
                  className="absolute top-0 left-0 h-full bg-gradient-to-r from-blue-500 to-indigo-500 transition-all duration-100"
                  style={{ width: `${progressPercent}%` }}
                />
              </div>

              {/* Controls Row */}
              <div className="flex items-center justify-between pt-1 flex-wrap gap-2">
                <div className="flex items-center gap-2 text-[10px] text-slate-300 font-mono">
                  <span>{isPlaying ? "▶ Playing" : "⏸ Paused"}</span>
                  {isInRoom && (
                    <>
                      <span>•</span>
                      <span className={`px-2 py-0.5 rounded font-bold ${
                        roomState?.driftSeconds !== undefined && Math.abs(roomState.driftSeconds) < 0.3
                          ? "bg-emerald-950 text-emerald-400 border border-emerald-800"
                          : roomState?.driftSeconds !== undefined && Math.abs(roomState.driftSeconds) <= 1.5
                          ? "bg-amber-950 text-amber-400 border border-amber-800"
                          : "bg-blue-950 text-blue-400 border border-blue-800"
                      }`}>
                        {roomState?.syncStatus || (roomState?.isHost ? "Authoritative Host" : "In Sync")}
                        {roomState?.driftSeconds !== undefined && !roomState.isHost && (
                          <span className="ml-1 opacity-80">
                            ({roomState.driftSeconds >= 0 ? `+${roomState.driftSeconds.toFixed(2)}s` : `${roomState.driftSeconds.toFixed(2)}s`})
                          </span>
                        )}
                      </span>
                    </>
                  )}
                </div>

                {/* Center Controls */}
                <div className="flex items-center gap-3">
                  <button
                    onClick={() => {
                      if (canJumpBackward) onSeek(-10, false);
                    }}
                    disabled={!canJumpBackward}
                    className={`p-1.5 rounded-lg transition active:scale-95 ${
                      canJumpBackward
                        ? "text-slate-400 hover:text-white hover:bg-slate-800"
                        : "text-slate-600 opacity-40 cursor-not-allowed"
                    }`}
                    title={canJumpBackward ? "Seek backward 10s" : "Rewind restricted by host"}
                  >
                    <RotateCcw className="w-4 h-4" />
                  </button>
                  <button
                    onClick={() => {
                      if (canPlayPause) onTogglePlay();
                    }}
                    disabled={!canPlayPause}
                    className={`w-10 h-10 rounded-full flex items-center justify-center transition active:scale-95 ${
                      canPlayPause
                        ? "bg-blue-600 hover:bg-blue-500 text-white shadow-md shadow-blue-500/30"
                        : "bg-slate-700 text-slate-500 opacity-50 cursor-not-allowed"
                    }`}
                    title={
                      !canPlayPause
                        ? "Play/Pause restricted by host"
                        : isPlaying
                        ? "Pause playback"
                        : "Resume playback"
                    }
                  >
                    {isPlaying ? (
                      <Pause className="w-4 h-4 fill-white" />
                    ) : (
                      <Play className="w-4 h-4 fill-white ml-0.5" />
                    )}
                  </button>
                  <button
                    onClick={() => {
                      if (canJumpForward) onSeek(10, false);
                    }}
                    disabled={!canJumpForward}
                    className={`p-1.5 rounded-lg transition active:scale-95 ${
                      canJumpForward
                        ? "text-slate-400 hover:text-white hover:bg-slate-800"
                        : "text-slate-600 opacity-40 cursor-not-allowed"
                    }`}
                    title={canJumpForward ? "Seek forward 10s" : "Fast-forward restricted by host"}
                  >
                    <RotateCw className="w-4 h-4" />
                  </button>
                </div>

                {/* Playback Speed Control */}
                <div className="flex items-center gap-1.5 text-xs text-slate-300">
                  <FastForward className="w-3.5 h-3.5 text-blue-400" />
                  <span className="text-[11px] text-slate-400">Speed:</span>
                  <div className="flex items-center bg-slate-800 rounded-lg p-0.5 border border-slate-700 font-mono text-[10px]">
                    {[0.75, 1.0, 1.25, 1.5].map((spd) => (
                      <button
                        key={spd}
                        disabled={!canSpeed}
                        onClick={() => {
                          if (canSpeed) onSpeedChange(spd);
                        }}
                        className={`px-1.5 py-0.5 rounded transition ${
                          Math.abs(mpvStatus.speed - spd) < 0.05
                            ? "bg-blue-600 text-white font-bold"
                            : canSpeed
                            ? "text-slate-400 hover:text-white"
                            : "text-slate-600 opacity-40 cursor-not-allowed"
                        }`}
                        title={canSpeed ? `Set playback speed to ${spd}x` : "Speed control restricted by host"}
                      >
                        {spd}x
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* ============================================================== */}
        {/* Right Column: Card 4 Connected Members (Col 5)                 */}
        {/* ============================================================== */}
        <div className="col-span-12 lg:col-span-5 flex flex-col gap-4">
          <div className="bg-white rounded-2xl border border-slate-200 p-5 shadow-2xs flex-1 flex flex-col gap-4">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Users className="w-4 h-4 text-blue-600" />
                <h3 className="text-xs font-bold uppercase tracking-wider text-slate-900">
                  Connected Members
                </h3>
              </div>
              <span className="text-xs text-slate-400 font-mono">
                {syncedCount}/{members.length} Synced
              </span>
            </div>

            {/* Member Cards List */}
            <div className="flex flex-col gap-2.5 max-h-[520px] overflow-y-auto pr-1">
              {members.length === 0 ? (
                <div className="flex flex-col items-center justify-center p-8 text-center text-slate-400 bg-slate-50/50 rounded-xl border border-dashed border-slate-200">
                  <Users className="w-8 h-8 text-slate-300 mb-2" />
                  <p className="text-xs font-semibold text-slate-600">No Connected Participants</p>
                  <p className="text-[11px] text-slate-400 mt-1 max-w-[220px]">
                    Host a room or share your sync:// link with peers on your Wi-Fi to watch together.
                  </p>
                </div>
              ) : (
                members.map((member) => {
                  const isSynced = Math.abs(member.driftSeconds) < 0.3 && member.movieMatched;
                  const isMinorDrift =
                    Math.abs(member.driftSeconds) >= 0.3 && Math.abs(member.driftSeconds) <= 1.5;

                  const getStatusBadge = () => {
                    switch (member.status) {
                      case "WAITING_FOR_MOVIE":
                        return { text: "Waiting for Movie", style: "bg-amber-100 text-amber-800 border-amber-200" };
                      case "VERIFYING_MOVIE":
                        return { text: "Verifying Movie...", style: "bg-cyan-100 text-cyan-800 border-cyan-200" };
                      case "LAUNCHING_PLAYER":
                        return { text: "Launching Player...", style: "bg-blue-100 text-blue-800 border-blue-200" };
                      case "LOADING_MOVIE":
                        return { text: "Loading Movie...", style: "bg-blue-100 text-blue-800 border-blue-200" };
                      case "WAITING_FOR_SYNC":
                        return { text: "Catching up with Host...", style: "bg-indigo-100 text-indigo-800 border-indigo-200" };
                      case "SYNCHRONIZING":
                        return { text: "Synchronizing...", style: "bg-indigo-100 text-indigo-800 border-indigo-200" };
                      case "WATCHING":
                        return isSynced
                          ? { text: "Watching (In Sync)", style: "bg-emerald-100 text-emerald-800 border-emerald-200" }
                          : isMinorDrift
                          ? { text: `Watching (Drift: ${member.driftSeconds.toFixed(2)}s)`, style: "bg-amber-100 text-amber-800 border-amber-200" }
                          : { text: "Watching (Out of Sync)", style: "bg-rose-100 text-rose-800 border-rose-200" };
                      case "OUT_OF_SYNC":
                        return { text: `Out of Sync (${member.driftSeconds.toFixed(2)}s)`, style: "bg-rose-100 text-rose-800 border-rose-200" };
                      case "ERROR":
                        return { text: "Error Loading", style: "bg-rose-100 text-rose-800 border-rose-200" };
                      case "DISCONNECTED":
                        return { text: "Disconnected", style: "bg-slate-100 text-slate-600 border-slate-200" };
                      default:
                        return { text: member.status || "Connected", style: "bg-slate-100 text-slate-700 border-slate-200" };
                    }
                  };

                  const badge = getStatusBadge();

                  return (
                    <div
                      key={member.id}
                      className="p-3 bg-slate-50 border border-slate-200/80 rounded-xl flex items-center justify-between text-xs"
                    >
                      <div className="min-w-0 pr-2">
                        <div className="flex items-center gap-2">
                          <span className="font-semibold text-slate-800 truncate">
                            {member.alias}
                          </span>
                          {member.isHost && (
                            <span className="text-[9px] font-bold bg-blue-100 text-blue-800 px-1.5 py-0.2 rounded uppercase">
                              Host
                            </span>
                          )}
                        </div>
                        <p className="text-[11px] text-slate-400 font-mono mt-0.5">
                          {member.ip} • Ping: {member.pingMs}ms
                          {member.position !== undefined && member.position > 0 && (
                            <span className="ml-1 text-slate-600 font-bold">
                              • Pos: {formatDuration(member.position)}
                            </span>
                          )}
                        </p>
                      </div>

                      <div className="text-right flex flex-col items-end flex-shrink-0">
                        <span
                          className={`text-[10px] font-bold px-2 py-0.5 rounded-full border ${badge.style}`}
                        >
                          {badge.text}
                        </span>
                        <span className="text-[10px] text-slate-400 font-mono mt-0.5">
                          {member.movieMatched ? "Movie Verified" : "Different File"}
                        </span>
                      </div>
                    </div>
                  );
                })
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
