import React, { useState, useRef } from "react";
import {
  Search,
  RotateCw,
  CheckSquare,
  UploadCloud,
  FileText,
  X,
  Send,
  Laptop,
  Smartphone,
  ArrowUpRight,
  ArrowDownLeft,
  FolderOpen,
  CheckCircle2,
  AlertTriangle,
  XCircle,
  Clock,
  HardDrive,
  FolderCog,
  File,
  Folder,
} from "lucide-react";
import type { PeerDevice, StagedFile, ActiveTransfer } from "../types";
import { pickFilesNative, pickFolderNative, resolveStagedFiles } from "../lib/tauriEvents";

interface WifiSendTabProps {
  peers: PeerDevice[];
  selectedPeerIds: string[];
  onTogglePeer: (id: string) => void;
  onSelectAllPeers: () => void;
  stagedFiles: StagedFile[];
  onAddFiles: (files: StagedFile[]) => void;
  onRemoveFile: (id: string) => void;
  onClearFiles: () => void;
  activeTransfers: ActiveTransfer[];
  completedTransfers: ActiveTransfer[];
  onInitiateSend: () => void;
  onScanPeers: () => void;
  onCancelTransfer?: (transferId: string) => void;
  localIp?: string;
  downloadDir?: string;
  onChangeDownloadDir?: () => void;
  onOpenDownloadFolder?: () => void;
}

export const WifiSendTab: React.FC<WifiSendTabProps> = ({
  peers,
  selectedPeerIds,
  onTogglePeer,
  onSelectAllPeers,
  stagedFiles,
  onAddFiles,
  onRemoveFile,
  onClearFiles,
  activeTransfers,
  completedTransfers,
  onInitiateSend,
  onScanPeers,
  onCancelTransfer,
  localIp = "127.0.0.1",
  downloadDir = "Downloads\\SyncDrop",
  onChangeDownloadDir,
  onOpenDownloadFolder,
}) => {
  const [searchQuery, setSearchQuery] = useState("");
  const [isDragging, setIsDragging] = useState(false);
  const [isScanning, setIsScanning] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Filter peers by search term
  const filteredPeers = peers.filter(
    (p) =>
      p.alias.toLowerCase().includes(searchQuery.toLowerCase()) ||
      p.deviceModel.toLowerCase().includes(searchQuery.toLowerCase()) ||
      p.ip.includes(searchQuery)
  );

  const formatFileSize = (bytes: number): string => {
    if (bytes >= 1024 * 1024 * 1024) return (bytes / (1024 * 1024 * 1024)).toFixed(2) + " GB";
    if (bytes >= 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + " MB";
    if (bytes >= 1024) return (bytes / 1024).toFixed(1) + " KB";
    return bytes + " B";
  };

  const handleBrowseFilesClick = async () => {
    try {
      // First try native Windows file picker dialog starting in download/movies folder
      const selectedPaths = await pickFilesNative(downloadDir);
      if (selectedPaths && selectedPaths.length > 0) {
        const resolved = await resolveStagedFiles(selectedPaths);
        onAddFiles(resolved);
        return;
      }
    } catch (err) {
      console.warn("Native file picker failed:", err);
    }

    if (fileInputRef.current) {
      fileInputRef.current.click();
    }
  };

  const handleBrowseFolderClick = async () => {
    try {
      // Opens folder picker to attach all files in a folder
      const selectedFolder = await pickFolderNative(downloadDir);
      if (selectedFolder) {
        const resolved = await resolveStagedFiles([selectedFolder]);
        if (resolved.length > 0) {
          onAddFiles(resolved);
        } else {
          alert("The selected folder does not contain any readable files.");
        }
      }
    } catch (err) {
      console.warn("Folder picker failed:", err);
    }
  };

  const handleFileInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      const newFiles: StagedFile[] = Array.from(e.target.files).map((file, idx) => ({
        id: `file-${Date.now()}-${idx}`,
        name: file.name,
        size: file.size,
        path: (file as unknown as { path?: string }).path || file.name,
      }));
      onAddFiles(newFiles);
    }
    if (e.target) e.target.value = "";
  };

  const handleDrop = async (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    setIsDragging(false);

    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      const paths: string[] = [];
      const fallbackFiles: StagedFile[] = [];

      for (let i = 0; i < e.dataTransfer.files.length; i++) {
        const file = e.dataTransfer.files[i];
        const rawPath = (file as unknown as { path?: string }).path;
        if (rawPath) {
          paths.push(rawPath);
        } else {
          fallbackFiles.push({
            id: `drop-${Date.now()}-${i}`,
            name: file.name,
            size: file.size,
          });
        }
      }

      if (paths.length > 0) {
        try {
          const resolved = await resolveStagedFiles(paths);
          onAddFiles(resolved);
          return;
        } catch (err) {
          console.warn("Could not resolve drag-and-drop paths directly:", err);
        }
      }

      if (fallbackFiles.length > 0) {
        onAddFiles(fallbackFiles);
      }
    }
  };

  const handleScanClick = () => {
    setIsScanning(true);
    onScanPeers();
    setTimeout(() => setIsScanning(false), 1200);
  };

  const totalStagedSize = stagedFiles.reduce((acc, f) => acc + f.size, 0);

  return (
    <div className="flex-1 flex flex-col overflow-hidden bg-slate-50/60 select-none">
      {/* Hidden HTML file input for web fallback */}
      <input
        type="file"
        ref={fileInputRef}
        onChange={handleFileInputChange}
        className="hidden"
        multiple
      />

      <div className="flex-1 grid grid-cols-12 gap-6 p-6 overflow-y-auto">
        {/* ============================================================== */}
        {/* Left Column: Discovered Devices & Search (Col 6)               */}
        {/* ============================================================== */}
        <div className="col-span-12 lg:col-span-6 flex flex-col gap-4">
          {/* Section Header */}
          <div className="flex items-center justify-between flex-wrap gap-2">
            <div>
              <div className="flex items-center gap-2">
                <h2 className="text-sm font-bold text-slate-900 tracking-tight">
                  Discovered Friends on LAN
                </h2>
                <span className="text-[10px] font-bold bg-blue-50 text-blue-700 px-2 py-0.5 rounded-full border border-blue-200">
                  {peers.filter((p) => !p.isStale).length} Online
                </span>
              </div>
              <p className="text-xs text-slate-500 mt-0.5">
                Select one or multiple peers to stream files directly over high-speed Wi-Fi.
              </p>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={onSelectAllPeers}
                className="flex items-center gap-1.5 bg-white hover:bg-slate-100 border border-slate-200 px-3 py-1.5 rounded-xl text-xs font-semibold text-slate-700 shadow-2xs transition active:scale-95"
              >
                <CheckSquare className="w-3.5 h-3.5 text-blue-600" />
                <span>Select All</span>
              </button>
              <button
                onClick={handleScanClick}
                className="flex items-center gap-1.5 bg-white hover:bg-slate-100 border border-slate-200 px-3 py-1.5 rounded-xl text-xs font-semibold text-slate-700 shadow-2xs transition active:scale-95"
              >
                <RotateCw
                  className={`w-3.5 h-3.5 text-slate-600 ${isScanning ? "animate-spin text-blue-600" : ""}`}
                />
                <span>Scan</span>
              </button>
            </div>
          </div>

          {/* Search Devices Input */}
          <div className="relative">
            <Search className="w-4 h-4 text-slate-400 absolute left-3.5 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              placeholder="Search by nickname, device model, or IP address..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full bg-white border border-slate-200/90 rounded-xl pl-10 pr-4 py-2 text-xs text-slate-800 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 transition shadow-2xs"
            />
          </div>

          {/* Discovered Device Cards List */}
          <div className="flex flex-col gap-2.5">
            {filteredPeers.length === 0 ? (
              <div className="flex flex-col items-center justify-center p-8 bg-white border border-dashed border-slate-200 rounded-2xl text-center">
                <div className="relative flex items-center justify-center w-12 h-12 mb-3">
                  <div className="absolute w-12 h-12 rounded-full bg-blue-100 animate-ping opacity-35" />
                  <div className="w-10 h-10 rounded-full bg-blue-50 text-blue-600 border border-blue-200 flex items-center justify-center shadow-xs">
                    <Laptop className="w-5 h-5" />
                  </div>
                </div>
                <h4 className="text-xs font-bold text-slate-800">
                  Scanning for SyncDrop peers on LAN...
                </h4>
                <p className="text-[11px] text-slate-500 max-w-sm mt-1">
                  SyncDrop automatically broadcasts UDP multicast on{" "}
                  <code className="text-blue-600 font-mono">224.0.0.167:53317</code>. Ensure other
                  laptops are connected to the same Wi-Fi and have SyncDrop open.
                </p>
                <div className="flex items-center gap-2 mt-4 text-[10px] text-slate-400 font-mono">
                  <span>Your IP: {localIp}</span>
                  <span>•</span>
                  <span>Direct HTTP: 53317</span>
                </div>
              </div>
            ) : (
              filteredPeers.map((peer) => {
                const isSelected = selectedPeerIds.includes(peer.id);
                const isOnline = !peer.isStale;

                return (
                  <div
                    key={peer.id}
                    onClick={() => onTogglePeer(peer.id)}
                    className={`p-3.5 rounded-2xl border transition-all cursor-pointer flex items-center justify-between ${
                      isSelected
                        ? "bg-blue-50/80 border-blue-400 shadow-sm shadow-blue-500/5 ring-1 ring-blue-400"
                        : "bg-white border-slate-200 hover:border-slate-300 shadow-2xs"
                    }`}
                  >
                    <div className="flex items-center gap-3.5 min-w-0">
                      {/* Checkbox indicator */}
                      <div
                        className={`w-5 h-5 rounded-lg flex items-center justify-center transition-all ${
                          isSelected
                            ? "bg-blue-600 text-white"
                            : "border border-slate-300 bg-white"
                        }`}
                      >
                        {isSelected && <CheckSquare className="w-3.5 h-3.5 stroke-[2.5]" />}
                      </div>

                      {/* Device Icon */}
                      <div
                        className={`w-10 h-10 rounded-xl flex items-center justify-center flex-shrink-0 ${
                          isSelected
                            ? "bg-blue-600 text-white shadow-md shadow-blue-500/20"
                            : "bg-slate-100 text-slate-600"
                        }`}
                      >
                        {peer.deviceType === "mobile" ? (
                          <Smartphone className="w-5 h-5" />
                        ) : (
                          <Laptop className="w-5 h-5" />
                        )}
                      </div>

                      {/* Peer Details */}
                      <div className="truncate">
                        <div className="flex items-center gap-2">
                          <span className="font-bold text-xs text-slate-900 truncate">
                            {peer.alias}
                          </span>
                          <span
                            className={`text-[9px] font-bold px-1.5 py-0.5 rounded-md uppercase tracking-wider ${
                              isOnline
                                ? "bg-emerald-50 text-emerald-700 border border-emerald-200"
                                : "bg-amber-50 text-amber-700 border border-amber-200"
                            }`}
                          >
                            {isOnline ? "Online" : "Stale"}
                          </span>
                        </div>
                        <p className="text-[11px] text-slate-500 truncate mt-0.5">
                          {peer.deviceModel}
                        </p>
                      </div>
                    </div>

                    {/* IP and Port Badge */}
                    <div className="text-right flex flex-col items-end flex-shrink-0 ml-3">
                      <span className="font-mono text-xs font-semibold text-slate-700">
                        {peer.ip}
                      </span>
                      <span className="text-[10px] text-slate-400 font-mono mt-0.5">
                        TCP:{peer.port}
                      </span>
                    </div>
                  </div>
                );
              })
            )}
          </div>

          {/* Download Destination Settings Card */}
          <div className="mt-2 p-4 bg-white rounded-2xl border border-slate-200 shadow-2xs flex flex-col gap-2.5">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <FolderCog className="w-4 h-4 text-slate-600" />
                <span className="text-xs font-bold text-slate-800">
                  Received Files Destination
                </span>
              </div>
              <div className="flex items-center gap-2">
                {onChangeDownloadDir && (
                  <button
                    onClick={onChangeDownloadDir}
                    className="text-[11px] font-semibold text-blue-600 hover:text-blue-700 transition"
                  >
                    Change Folder
                  </button>
                )}
                {onOpenDownloadFolder && (
                  <button
                    onClick={onOpenDownloadFolder}
                    className="flex items-center gap-1 text-[11px] font-semibold text-slate-600 hover:text-slate-800 transition"
                  >
                    <FolderOpen className="w-3.5 h-3.5" />
                    <span>Open</span>
                  </button>
                )}
              </div>
            </div>
            <div className="flex items-center gap-2 px-3 py-2 bg-slate-50 border border-slate-200/80 rounded-xl text-xs font-mono text-slate-600 truncate">
              <HardDrive className="w-3.5 h-3.5 text-slate-400 flex-shrink-0" />
              <span className="truncate">{downloadDir}</span>
            </div>
          </div>
        </div>

        {/* ============================================================== */}
        {/* Right Column: File Staging, Send & Transfer Status (Col 6)     */}
        {/* ============================================================== */}
        <div className="col-span-12 lg:col-span-6 flex flex-col gap-5">
          {/* Card 1: Staging & Sending Area */}
          <div className="bg-white rounded-2xl border border-slate-200 p-5 shadow-2xs flex flex-col gap-4">
            <div className="flex items-center justify-between">
              <span className="text-xs font-bold uppercase tracking-wider text-slate-900">
                Send Files
              </span>
              {stagedFiles.length > 0 && (
                <button
                  onClick={onClearFiles}
                  className="text-xs text-red-600 hover:text-red-700 font-medium transition"
                >
                  Clear all
                </button>
              )}
            </div>

            {/* Drag & Drop Box */}
            <div
              onDragOver={(e) => {
                e.preventDefault();
                setIsDragging(true);
              }}
              onDragLeave={() => setIsDragging(false)}
              onDrop={handleDrop}
              className={`border-2 border-dashed rounded-2xl p-6 flex flex-col items-center justify-center text-center transition-all ${
                isDragging
                  ? "border-blue-500 bg-blue-50/50 scale-[0.99]"
                  : "border-slate-200 bg-slate-50/50 hover:bg-slate-50 hover:border-slate-300"
              }`}
            >
              <div className="w-12 h-12 rounded-xl bg-blue-50 text-blue-600 flex items-center justify-center mb-3 shadow-inner">
                <UploadCloud className="w-6 h-6 stroke-[1.75]" />
              </div>
              <h3 className="text-xs font-bold text-slate-800">
                Drag and drop files here, or browse
              </h3>
              <p className="text-[11px] text-slate-500 max-w-xs mt-1">
                Movies, documents, folders, or archives. Supports multi-gigabyte files streamed
                directly without cloud relays.
              </p>
              <div className="flex items-center gap-2 mt-4">
                <button
                  onClick={handleBrowseFilesClick}
                  className="px-3.5 py-2 bg-white border border-slate-200 hover:bg-slate-100 rounded-xl text-xs font-semibold text-slate-700 shadow-2xs transition active:scale-95 flex items-center gap-1.5"
                >
                  <File className="w-3.5 h-3.5 text-blue-600" />
                  <span>Browse Files</span>
                </button>
                <button
                  onClick={handleBrowseFolderClick}
                  className="px-3.5 py-2 bg-white border border-slate-200 hover:bg-slate-100 rounded-xl text-xs font-semibold text-slate-700 shadow-2xs transition active:scale-95 flex items-center gap-1.5"
                >
                  <Folder className="w-3.5 h-3.5 text-amber-500" />
                  <span>Browse Folder</span>
                </button>
              </div>
            </div>

            {/* Staged Files List */}
            {stagedFiles.length > 0 && (
              <div className="flex flex-col gap-2">
                <div className="flex items-center justify-between text-xs text-slate-500 px-1 font-medium">
                  <span>
                    Selected {stagedFiles.length} file{stagedFiles.length > 1 ? "s" : ""}
                  </span>
                  <span className="font-mono font-semibold text-slate-700">
                    Total: {formatFileSize(totalStagedSize)}
                  </span>
                </div>
                <div className="max-h-40 overflow-y-auto flex flex-col gap-1.5 pr-1">
                  {stagedFiles.map((file) => (
                    <div
                      key={file.id}
                      className="flex items-center justify-between p-2.5 bg-slate-50 border border-slate-200/80 rounded-xl text-xs"
                    >
                      <div className="flex items-center gap-2.5 min-w-0">
                        <FileText className="w-4 h-4 text-blue-600 flex-shrink-0" />
                        <span className="font-semibold text-slate-800 truncate">{file.name}</span>
                      </div>
                      <div className="flex items-center gap-3 ml-2 flex-shrink-0">
                        <span className="font-mono text-slate-500 text-[11px]">
                          {formatFileSize(file.size)}
                        </span>
                        <button
                          onClick={() => onRemoveFile(file.id)}
                          className="text-slate-400 hover:text-red-500 transition"
                        >
                          <X className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Send Action Button */}
            <button
              onClick={onInitiateSend}
              disabled={stagedFiles.length === 0 || selectedPeerIds.length === 0}
              className={`w-full py-3 rounded-xl font-semibold text-xs flex items-center justify-center gap-2 shadow-sm transition active:scale-[0.98] ${
                stagedFiles.length > 0 && selectedPeerIds.length > 0
                  ? "bg-blue-600 hover:bg-blue-700 text-white shadow-blue-500/25"
                  : "bg-slate-100 text-slate-400 cursor-not-allowed"
              }`}
            >
              <Send className="w-4 h-4" />
              <span>
                {selectedPeerIds.length === 0
                  ? "Select at least 1 recipient"
                  : stagedFiles.length === 0
                  ? "Select files to send"
                  : `Send ${stagedFiles.length} file${stagedFiles.length > 1 ? "s" : ""} to ${selectedPeerIds.length} friend${selectedPeerIds.length > 1 ? "s" : ""}`}
              </span>
            </button>
          </div>

          {/* Card 2: Active Transfers Section */}
          <div className="bg-white rounded-2xl border border-slate-200 p-5 shadow-2xs flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-bold uppercase tracking-wider text-slate-900">
                Active Transfers
              </span>
              <span className="text-xs text-slate-400 font-mono">
                {activeTransfers.length} Active • Direct HTTP
              </span>
            </div>

            {/* Active Transfer Items */}
            {activeTransfers.length === 0 ? (
              <div className="py-6 text-center text-xs text-slate-400 border border-slate-100 rounded-xl bg-slate-50/50">
                No active transfers in progress. Select peers and files above to start.
              </div>
            ) : (
              <div className="flex flex-col gap-3">
                {activeTransfers.map((t) => {
                  const percent = Math.min(
                    100,
                    Math.round((t.bytesTransferred / (t.totalBytes || 1)) * 100)
                  );
                  const isOutgoing = t.direction === "outgoing";
                  const isWaiting = t.status === "waiting_response";

                  return (
                    <div
                      key={t.transferId + t.fileId}
                      className="p-3.5 bg-slate-50 border border-slate-200/90 rounded-xl flex flex-col gap-2.5"
                    >
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2.5 min-w-0">
                          <div
                            className={`w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0 ${
                              isOutgoing
                                ? "bg-blue-100 text-blue-700"
                                : "bg-emerald-100 text-emerald-700"
                            }`}
                          >
                            {isOutgoing ? (
                              <ArrowUpRight className="w-4 h-4 stroke-[2.5]" />
                            ) : (
                              <ArrowDownLeft className="w-4 h-4 stroke-[2.5]" />
                            )}
                          </div>
                          <div className="truncate">
                            <span className="font-semibold text-xs text-slate-800 truncate block">
                              {t.filename}
                            </span>
                            <p className="text-[11px] text-slate-400">
                              {isOutgoing ? `To: ${t.peerAlias}` : `From: ${t.peerAlias}`} •{" "}
                              {formatFileSize(t.totalBytes)}
                            </p>
                          </div>
                        </div>

                        <div className="text-right flex flex-col items-end flex-shrink-0">
                          <div className="flex items-center gap-1.5">
                            <span
                              className={`text-[10px] font-bold px-2 py-0.5 rounded-full ${
                                isWaiting
                                  ? "bg-amber-100 text-amber-800 border border-amber-200"
                                  : "bg-blue-100 text-blue-800 border border-blue-200"
                              }`}
                            >
                              {isWaiting ? "Waiting for recipient..." : "Transferring"}
                            </span>
                            {onCancelTransfer && (
                              <button
                                onClick={() => onCancelTransfer(t.transferId)}
                                title="Cancel transfer"
                                className="w-5 h-5 rounded-full flex items-center justify-center text-slate-400 hover:text-red-600 hover:bg-red-50 transition"
                              >
                                <X className="w-3.5 h-3.5" />
                              </button>
                            )}
                          </div>
                          {!isWaiting && (
                            <span className="text-[11px] font-mono font-medium text-slate-600 mt-0.5">
                              {formatFileSize(t.speedBytesPerSec)}/s
                            </span>
                          )}
                        </div>
                      </div>

                      {/* Smooth Progress Bar */}
                      <div className="w-full bg-slate-200 rounded-full h-2 overflow-hidden">
                        <div
                          className="h-full bg-gradient-to-r from-blue-600 to-indigo-600 transition-all duration-200"
                          style={{ width: `${percent}%` }}
                        />
                      </div>
                      <div className="flex items-center justify-between text-[10px] text-slate-500 font-mono">
                        <span>
                          {formatFileSize(t.bytesTransferred)} of {formatFileSize(t.totalBytes)} (
                          {percent}%)
                        </span>
                        <span>{isWaiting ? "Prompt sent" : `${100 - percent}% remaining`}</span>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          {/* Card 3: Completed Transfers History Section */}
          <div className="bg-white rounded-2xl border border-slate-200 p-5 shadow-2xs flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <CheckCircle2 className="w-4 h-4 text-emerald-600" />
                <span className="text-xs font-bold uppercase tracking-wider text-slate-900">
                  Transfer History
                </span>
              </div>
              <span className="text-xs text-slate-400 font-medium">
                {completedTransfers.length} recorded
              </span>
            </div>

            {completedTransfers.length === 0 ? (
              <div className="py-4 text-center text-xs text-slate-400 border border-slate-100 rounded-xl bg-slate-50/50">
                No transfer history recorded in this session.
              </div>
            ) : (
              <div className="flex flex-col gap-2 max-h-48 overflow-y-auto pr-1">
                {completedTransfers.map((item, idx) => {
                  const isSuccess = item.status === "completed";
                  const isDeclined = item.status === "declined";
                  const isCancelled = item.status === "cancelled";

                  return (
                    <div
                      key={item.transferId + idx}
                      className="flex items-center justify-between p-2.5 bg-slate-50/80 border border-slate-200/70 rounded-xl text-xs"
                    >
                      <div className="flex items-center gap-2.5 min-w-0">
                        {isSuccess ? (
                          <CheckCircle2 className="w-4 h-4 text-emerald-500 flex-shrink-0" />
                        ) : isDeclined ? (
                          <AlertTriangle className="w-4 h-4 text-amber-500 flex-shrink-0" />
                        ) : isCancelled ? (
                          <Clock className="w-4 h-4 text-slate-400 flex-shrink-0" />
                        ) : (
                          <XCircle className="w-4 h-4 text-red-500 flex-shrink-0" />
                        )}

                        <div className="truncate">
                          <p className="font-semibold text-slate-800 truncate">{item.filename}</p>
                          <p className="text-[10px] text-slate-400">
                            {item.direction === "outgoing" ? "Sent to" : "Received from"}{" "}
                            {item.peerAlias} • {formatFileSize(item.totalBytes)}
                            {item.errorMessage && (
                              <span className="text-red-500 ml-1">({item.errorMessage})</span>
                            )}
                          </p>
                        </div>
                      </div>

                      <div className="flex items-center gap-2 flex-shrink-0 ml-2">
                        {isSuccess && onOpenDownloadFolder && item.direction === "incoming" && (
                          <button
                            onClick={onOpenDownloadFolder}
                            className="p-1 text-slate-400 hover:text-blue-600 transition"
                            title="Show in Explorer"
                          >
                            <FolderOpen className="w-3.5 h-3.5" />
                          </button>
                        )}
                        <span
                          className={`text-[10px] font-mono px-2 py-0.5 rounded-full font-medium ${
                            isSuccess
                              ? "text-emerald-700 bg-emerald-50 border border-emerald-200"
                              : isDeclined
                              ? "text-amber-700 bg-amber-50 border border-amber-200"
                              : isCancelled
                              ? "text-slate-600 bg-slate-100 border border-slate-200"
                              : "text-red-700 bg-red-50 border border-red-200"
                          }`}
                        >
                          {isSuccess
                            ? "Success"
                            : isDeclined
                            ? "Declined"
                            : isCancelled
                            ? "Cancelled"
                            : "Failed"}
                        </span>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Footer Status Bar */}
      <footer className="px-6 py-2.5 bg-white border-t border-slate-200 text-xs text-slate-500 flex items-center justify-between flex-shrink-0 flex-wrap gap-2">
        <div className="flex items-center gap-3 text-[11px]">
          <span className="flex items-center gap-1 font-medium text-slate-700">
            <span className="text-amber-500 font-bold">⚡</span> Zero-Config UDP Discovery
          </span>
          <span className="text-slate-300">•</span>
          <span className="flex items-center gap-1 font-medium text-slate-700">
            <span className="text-blue-600 font-bold">🔒</span> Direct TCP File Streaming
          </span>
          <span className="text-slate-300">•</span>
          <span className="font-mono text-slate-500">Multicast: 224.0.0.167</span>
        </div>
        <div className="flex items-center gap-2 text-[11px] font-mono text-slate-500">
          <span>Local IP: {localIp}</span>
        </div>
      </footer>
    </div>
  );
};
