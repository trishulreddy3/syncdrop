import { useState, useEffect } from "react";
import { TopBar } from "./components/TopBar";
import { WifiSendTab } from "./components/WifiSendTab";
import { WatchPartyTab } from "./components/WatchPartyTab";
import { TransferPromptModal } from "./components/TransferPromptModal";
import { UpdateModal } from "./components/UpdateModal";
import {
  getSystemInfo,
  getLocalDeviceInfo,
  getDiscoveredPeers,
  setLocalAlias,
  refreshPeerDiscovery,
  onPeersUpdated,
  onIncomingTransfer,
  onTransferProgress,
  sendFilesToRecipients,
  respondToTransferPrompt,
  cancelTransfer,
  getDownloadDirectory,
  setDownloadDirectory,
  openDownloadFolder,
  pickDownloadDirectoryNative,
  getMpvStatus,
  onMpvStatusChanged,
  launchMpv,
  togglePauseMpv,
  seekMpv,
  setMpvSpeed,
  stopMpv,
  hostWatchRoom,
  joinWatchRoom,
  leaveWatchRoom,
  getWatchRoomState,
  dispatchRoomPlay,
  dispatchRoomPause,
  dispatchRoomSeek,
  dispatchRoomSpeed,
  setRoomMovie,
  setLocalWatchMovie,
  syncWatchRoom,
  getRoomMembers,
  getDiscoveredRooms,
  onRoomStateSync,
  onRoomMembersUpdated,
  onRoomFileInfo,
  onDiscoveredRoomsUpdated,
  onRoomDisconnected,
  setRoomPermissions,
  checkForUpdates,
} from "./lib/tauriEvents";
import type {
  ActiveTab,
  PeerDevice,
  StagedFile,
  ActiveTransfer,
  IncomingTransferPrompt,
  RoomMember,
  MovieFileInfo,
  DiscoveredRoom,
  MpvPlaybackStatus,
  WatchRoomState,
  RoomControlPermissions,
  UpdateInfo,
} from "./types";

export function App() {
  const [activeTab, setActiveTab] = useState<ActiveTab>("wifi-send");
  const [nickname, setNickname] = useState<string>(() => {
    return localStorage.getItem("syncdrop_nickname") || "SyncDrop-User";
  });
  const [localIp, setLocalIp] = useState("127.0.0.1");
  const [downloadDir, setDownloadDir] = useState("Downloads\\SyncDrop");

  // Real LAN Discovered peers state (Zero mock peers)
  const [peers, setPeers] = useState<PeerDevice[]>([]);
  const [selectedPeerIds, setSelectedPeerIds] = useState<string[]>([]);

  // Staged files for transfer
  const [stagedFiles, setStagedFiles] = useState<StagedFile[]>([]);

  // Active and completed transfers (Real-time byte tracking)
  const [activeTransfers, setActiveTransfers] = useState<ActiveTransfer[]>([]);
  const [completedTransfers, setCompletedTransfers] = useState<ActiveTransfer[]>([]);

  // Incoming transfer prompt modal state
  const [incomingPrompt, setIncomingPrompt] = useState<IncomingTransferPrompt | null>(null);

  // mpv media engine real-time state
  const [mpvStatus, setMpvStatus] = useState<MpvPlaybackStatus>({
    engineStatus: "uninitialized",
    timePos: 0,
    duration: 0,
    pause: true,
    speed: 1.0,
  });

  // Watch Party states (Real WebSocket networking)
  const [roomState, setRoomState] = useState<WatchRoomState | null>(null);
  const [selectedMovie, setSelectedMovie] = useState<MovieFileInfo | null>(null);
  const [members, setMembers] = useState<RoomMember[]>([]);
  const [discoveredRooms, setDiscoveredRooms] = useState<DiscoveredRoom[]>([]);

  // In-App Update State
  const [availableUpdate, setAvailableUpdate] = useState<UpdateInfo | null>(null);
  const [showUpdateModal, setShowUpdateModal] = useState(false);
  const [isCheckingUpdates, setIsCheckingUpdates] = useState(false);

  // Initialize native LAN discovery & fetch local device info
  useEffect(() => {
    // 1. Probe system info
    getSystemInfo()
      .then((info) => console.log("SyncDrop Core System:", info))
      .catch(console.warn);

    // Check for software updates in background
    checkForUpdates()
      .then((info) => {
        if (info.available) {
          setAvailableUpdate(info);
        }
      })
      .catch(() => {});

    // 2. Fetch dynamic physical IP & local device info
    getLocalDeviceInfo()
      .then((deviceInfo) => {
        setLocalIp(deviceInfo.ip);
        const savedNickname = localStorage.getItem("syncdrop_nickname");
        if (savedNickname) {
          setNickname(savedNickname);
          setLocalAlias(savedNickname).catch(console.warn);
        } else if (deviceInfo.alias) {
          setNickname(deviceInfo.alias);
        }
      })
      .catch((err) => {
        console.warn("Could not retrieve local device info:", err);
      });

    // 3. Fetch current download directory
    getDownloadDirectory()
      .then((dir) => setDownloadDir(dir))
      .catch(console.warn);

    // 4. Fetch currently discovered peers and watch rooms
    getDiscoveredPeers()
      .then((currentPeers) => {
        setPeers(currentPeers);
      })
      .catch((err) => {
        console.warn("Could not retrieve initial peers:", err);
      });

    getDiscoveredRooms()
      .then((rooms) => setDiscoveredRooms(rooms))
      .catch(console.warn);

    // 5. Fetch initial mpv playback status and room state
    getMpvStatus()
      .then((status) => setMpvStatus(status))
      .catch(console.warn);

    getWatchRoomState()
      .then((st) => {
        if (st && st.roomId) {
          setRoomState(st);
          getRoomMembers().then(setMembers).catch(console.warn);
        }
      })
      .catch(console.warn);

    // 6. Subscribe to real-time peer registry events
    let unlistenPeers: (() => void) | undefined;
    onPeersUpdated((updatedPeers) => {
      setPeers(updatedPeers);
      getDiscoveredRooms().then(setDiscoveredRooms).catch(console.warn);
    })
      .then((unlistenFn) => {
        unlistenPeers = unlistenFn;
      })
      .catch((err) => {
        console.warn("Could not attach peer registry listener:", err);
      });

    // 7. Subscribe to real-time incoming transfer prompts
    let unlistenIncoming: (() => void) | undefined;
    onIncomingTransfer((prompt) => {
      console.log("Incoming transfer prompt received:", prompt);
      setIncomingPrompt(prompt);
    })
      .then((unlistenFn) => {
        unlistenIncoming = unlistenFn;
      })
      .catch((err) => {
        console.warn("Could not attach incoming transfer listener:", err);
      });

    // 8. Subscribe to real-time transfer progress updates (incoming & outgoing)
    let unlistenProgress: (() => void) | undefined;
    onTransferProgress((update) => {
      const isTerminal = ["completed", "failed", "declined", "cancelled"].includes(update.status);

      if (isTerminal) {
        // Move from active to completed history
        setActiveTransfers((prev) =>
          prev.filter(
            (t) => !(t.transferId === update.transferId && t.fileId === update.fileId)
          )
        );
        setCompletedTransfers((prev) => [
          update,
          ...prev.filter(
            (t) => !(t.transferId === update.transferId && t.fileId === update.fileId)
          ),
        ]);
      } else {
        // Active streaming or waiting
        setActiveTransfers((prev) => {
          const idx = prev.findIndex(
            (t) => t.transferId === update.transferId && t.fileId === update.fileId
          );
          if (idx >= 0) {
            const next = [...prev];
            next[idx] = update;
            return next;
          }
          return [update, ...prev];
        });
      }
    })
      .then((unlistenFn) => {
        unlistenProgress = unlistenFn;
      })
      .catch((err) => {
        console.warn("Could not attach transfer progress listener:", err);
      });

    // 9. Subscribe to mpv status property updates
    let unlistenMpv: (() => void) | undefined;
    onMpvStatusChanged((status) => {
      setMpvStatus(status);
    })
      .then((unlistenFn) => {
        unlistenMpv = unlistenFn;
      })
      .catch((err) => {
        console.warn("Could not attach mpv status listener:", err);
      });

    // 10. Watch Party WebSocket & Sync Room Listeners
    let unlistenSync: (() => void) | undefined;
    onRoomStateSync((state) => {
      if (state && state.roomId && state.roomId.trim().length > 0) {
        setRoomState(state);
        if (state.movieInfo) {
          setSelectedMovie(state.movieInfo);
        }
      } else {
        setRoomState(null);
        setMembers([]);
      }
    })
      .then((fn) => {
        unlistenSync = fn;
      })
      .catch(console.warn);

    let unlistenMembers: (() => void) | undefined;
    onRoomMembersUpdated((updatedMembers) => {
      setMembers(updatedMembers);
    })
      .then((fn) => {
        unlistenMembers = fn;
      })
      .catch(console.warn);

    let unlistenFileInfo: (() => void) | undefined;
    onRoomFileInfo((info) => {
      setSelectedMovie(info);
    })
      .then((fn) => {
        unlistenFileInfo = fn;
      })
      .catch(console.warn);

    let unlistenDiscoveredRooms: (() => void) | undefined;
    onDiscoveredRoomsUpdated((rooms) => {
      setDiscoveredRooms(rooms);
    })
      .then((fn) => {
        unlistenDiscoveredRooms = fn;
      })
      .catch(console.warn);

    let unlistenRoomDisconnected: (() => void) | undefined;
    onRoomDisconnected(() => {
      setRoomState(null);
      setMembers([]);
    })
      .then((fn) => {
        unlistenRoomDisconnected = fn;
      })
      .catch(console.warn);

    return () => {
      if (unlistenPeers) unlistenPeers();
      if (unlistenIncoming) unlistenIncoming();
      if (unlistenProgress) unlistenProgress();
      if (unlistenMpv) unlistenMpv();
      if (unlistenSync) unlistenSync();
      if (unlistenMembers) unlistenMembers();
      if (unlistenFileInfo) unlistenFileInfo();
      if (unlistenDiscoveredRooms) unlistenDiscoveredRooms();
      if (unlistenRoomDisconnected) unlistenRoomDisconnected();
    };
  }, []);

  const handleNicknameChange = (newName: string) => {
    setNickname(newName);
    localStorage.setItem("syncdrop_nickname", newName);
    setLocalAlias(newName).catch(console.warn);
  };

  const handleTogglePeer = (id: string) => {
    setSelectedPeerIds((prev) =>
      prev.includes(id) ? prev.filter((p) => p !== id) : [...prev, id]
    );
  };

  const handleSelectAllPeers = () => {
    if (selectedPeerIds.length === peers.length) {
      setSelectedPeerIds([]);
    } else {
      setSelectedPeerIds(peers.map((p) => p.id));
    }
  };

  const handleAddFiles = (newFiles: StagedFile[]) => {
    setStagedFiles((prev) => [...prev, ...newFiles]);
  };

  const handleRemoveFile = (id: string) => {
    setStagedFiles((prev) => prev.filter((f) => f.id !== id));
  };

  const handleClearFiles = () => {
    setStagedFiles([]);
  };

  const handleInitiateSend = async () => {
    if (stagedFiles.length === 0 || selectedPeerIds.length === 0) return;

    const filePaths = stagedFiles.map((f) => f.path || f.name).filter(Boolean);

    try {
      await sendFilesToRecipients(filePaths, selectedPeerIds);
    } catch (err) {
      console.error("Failed to initiate transfer:", err);
    }
  };

  const handleCancelTransfer = async (transferId: string) => {
    try {
      await cancelTransfer(transferId);
    } catch (err) {
      console.warn("Could not cancel transfer:", err);
    }
  };

  const handleScanPeers = () => {
    refreshPeerDiscovery()
      .then((latestPeers) => {
        setPeers(latestPeers);
      })
      .catch(console.warn);
  };

  const handleAcceptTransfer = (transferId: string) => {
    respondToTransferPrompt(transferId, true).catch(console.error);
    setIncomingPrompt(null);
  };

  const handleDeclineTransfer = (transferId: string) => {
    respondToTransferPrompt(transferId, false).catch(console.error);
    setIncomingPrompt(null);
  };

  const handleChangeDownloadDir = async () => {
    const selected = await pickDownloadDirectoryNative();
    if (selected) {
      await setDownloadDirectory(selected);
      setDownloadDir(selected);
    }
  };

  const handleOpenDownloadFolder = async () => {
    try {
      await openDownloadFolder();
    } catch (err) {
      console.warn("Could not open download folder:", err);
    }
  };

  // mpv & Watch Party Playback Control Handlers
  const isInRoom = Boolean(roomState && roomState.roomId);

  const handleTogglePlay = async () => {
    try {
      if (isInRoom) {
        if (mpvStatus.pause) {
          await dispatchRoomPlay();
        } else {
          await dispatchRoomPause();
        }
      } else {
        await togglePauseMpv();
      }
    } catch (err) {
      console.warn("Could not toggle playback:", err);
    }
  };

  const handleSeek = async (seconds: number, absolute: boolean = true) => {
    try {
      if (isInRoom) {
        const targetPos = absolute ? seconds : Math.max(0, mpvStatus.timePos + seconds);
        const seekType = absolute ? "absolute" : seconds >= 0 ? "forward" : "backward";
        await dispatchRoomSeek(targetPos, seekType);
      } else {
        await seekMpv(seconds, absolute);
      }
    } catch (err) {
      console.warn("Could not seek playback:", err);
    }
  };

  const handleUpdatePermissions = async (permissions: RoomControlPermissions) => {
    try {
      await setRoomPermissions(permissions);
    } catch (err) {
      console.warn("Could not update room control permissions:", err);
    }
  };

  const handleSpeedChange = async (speed: number) => {
    try {
      if (isInRoom) {
        await dispatchRoomSpeed(speed);
      } else {
        await setMpvSpeed(speed);
      }
    } catch (err) {
      console.warn("Could not set playback speed:", err);
    }
  };

  const handleSelectMovie = async (movie: MovieFileInfo) => {
    setSelectedMovie(movie);
    if (isInRoom && roomState?.isHost) {
      try {
        await setRoomMovie(movie);
      } catch (err) {
        console.warn("Could not update room movie:", err);
      }
    } else if (isInRoom && !roomState?.isHost) {
      try {
        await setLocalWatchMovie(movie);
      } catch (err) {
        console.warn("Could not set local watch movie:", err);
      }
    }
  };

  const handleHostRoom = async () => {
    try {
      const state = await hostWatchRoom(
        `${nickname}'s Watch Party`,
        selectedMovie || undefined
      );
      setRoomState(state);
      const initialMembers = await getRoomMembers();
      setMembers(initialMembers);
      const updatedRooms = await getDiscoveredRooms();
      setDiscoveredRooms(updatedRooms);
    } catch (err: any) {
      console.error("Failed to host watch room:", err);
      const errMsg = typeof err === "string" ? err : err?.message || "Failed to create watch party room.";
      alert(`Host Room Failed: ${errMsg}`);
    }
  };

  const handleJoinRoom = async (uri: string) => {
    try {
      await joinWatchRoom(uri);
      const state = await getWatchRoomState();
      if (state && state.roomId && state.roomId.trim().length > 0) {
        setRoomState(state);
        const initialMembers = await getRoomMembers();
        setMembers(initialMembers);
      } else {
        setRoomState(null);
        setMembers([]);
      }
    } catch (err: any) {
      console.error("Failed to join watch room:", err);
      const errMsg = typeof err === "string" ? err : err?.message || "Could not connect to host. Make sure the host is running and the room is active.";
      alert(`Join Room Failed: ${errMsg}`);
      setRoomState(null);
      setMembers([]);
    }
  };

  const handleLeaveRoom = async () => {
    try {
      await leaveWatchRoom();
      setRoomState(null);
      setMembers([]);
      const updatedRooms = await getDiscoveredRooms();
      setDiscoveredRooms(updatedRooms);
    } catch (err) {
      console.warn("Failed to leave room:", err);
      setRoomState(null);
      setMembers([]);
    }
  };

  const handleSyncRoom = async () => {
    try {
      await syncWatchRoom();
    } catch (err) {
      console.warn("Failed to request room sync:", err);
    }
  };

  const handleSimulateDrift = async () => {
    try {
      // Simulate an intentional 0.8s offset to trigger drift correction
      const current = mpvStatus.timePos;
      await seekMpv(Math.max(0, current - 0.8), true);
    } catch (err) {
      console.warn("Could not simulate drift event:", err);
    }
  };

  const handleLaunchPlayer = async (filePath?: string) => {
    try {
      await launchMpv(filePath);
    } catch (err) {
      console.error("Could not launch mpv sidecar:", err);
    }
  };

  const handleStopPlayer = async () => {
    try {
      await stopMpv();
    } catch (err) {
      console.warn("Could not stop mpv sidecar:", err);
    }
  };

  const handleCheckUpdates = async () => {
    setIsCheckingUpdates(true);
    try {
      const info = await checkForUpdates();
      if (info.available) {
        setAvailableUpdate(info);
        setShowUpdateModal(true);
      } else {
        alert(
          `You are already running the latest version of SyncDrop (v${info.currentVersion})!`
        );
      }
    } catch (err: any) {
      alert(
        typeof err === "string"
          ? err
          : "Could not connect to update server. Please check your internet connection."
      );
    } finally {
      setIsCheckingUpdates(false);
    }
  };

  return (
    <div className="w-screen h-screen flex flex-col bg-slate-50 overflow-hidden font-sans">
      {/* Top Header & Navigation Bar */}
      <TopBar
        activeTab={activeTab}
        onTabChange={setActiveTab}
        nickname={nickname}
        onNicknameChange={handleNicknameChange}
        localIp={localIp}
        onlinePeerCount={peers.filter((p) => !p.isStale).length}
        onOpenSettings={handleCheckUpdates}
        isDemoMode={false}
        availableUpdate={availableUpdate}
        onOpenUpdateModal={() => setShowUpdateModal(true)}
        onCheckUpdates={handleCheckUpdates}
        isCheckingUpdates={isCheckingUpdates}
      />

      {/* Main Tab Content */}
      <main className="flex-1 flex flex-col overflow-hidden">
        {activeTab === "wifi-send" ? (
          <WifiSendTab
            peers={peers}
            selectedPeerIds={selectedPeerIds}
            onTogglePeer={handleTogglePeer}
            onSelectAllPeers={handleSelectAllPeers}
            stagedFiles={stagedFiles}
            onAddFiles={handleAddFiles}
            onRemoveFile={handleRemoveFile}
            onClearFiles={handleClearFiles}
            activeTransfers={activeTransfers}
            completedTransfers={completedTransfers}
            onInitiateSend={handleInitiateSend}
            onScanPeers={handleScanPeers}
            onCancelTransfer={handleCancelTransfer}
            localIp={localIp}
            downloadDir={downloadDir}
            onChangeDownloadDir={handleChangeDownloadDir}
            onOpenDownloadFolder={handleOpenDownloadFolder}
          />
        ) : (
          <WatchPartyTab
            roomState={roomState}
            members={members}
            selectedMovie={selectedMovie}
            discoveredRooms={discoveredRooms}
            mpvStatus={mpvStatus}
            localIp={localIp}
            downloadDir={downloadDir}
            completedTransfers={completedTransfers}
            onSwitchToWifiSend={() => setActiveTab("wifi-send")}
            onSelectMovie={handleSelectMovie}
            onTogglePlay={handleTogglePlay}
            onSeek={handleSeek}
            onSpeedChange={handleSpeedChange}
            onLaunchPlayer={handleLaunchPlayer}
            onStopPlayer={handleStopPlayer}
            onHostRoom={handleHostRoom}
            onLeaveRoom={handleLeaveRoom}
            onJoinRoom={handleJoinRoom}
            onSyncRoom={handleSyncRoom}
            onSimulateDrift={handleSimulateDrift}
            onUpdatePermissions={handleUpdatePermissions}
            isDemoMode={false}
          />
        )}
      </main>

      {/* Incoming Transfer Prompt Modal */}
      {incomingPrompt && (
        <TransferPromptModal
          prompt={incomingPrompt}
          onAccept={handleAcceptTransfer}
          onDecline={handleDeclineTransfer}
        />
      )}

      {/* In-App Auto-Update & Installer Modal */}
      {showUpdateModal && availableUpdate && (
        <UpdateModal
          updateInfo={availableUpdate}
          onClose={() => setShowUpdateModal(false)}
        />
      )}
    </div>
  );
}

export default App;
