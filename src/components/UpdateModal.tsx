import React, { useState, useEffect } from "react";
import {
  Download,
  CheckCircle,
  AlertCircle,
  ExternalLink,
  X,
  Sparkles,
  ArrowRight,
  RefreshCw,
} from "lucide-react";
import type { UpdateInfo, UpdateProgress } from "../types";
import { downloadAndInstallUpdate, onUpdateProgress } from "../lib/tauriEvents";

interface UpdateModalProps {
  updateInfo: UpdateInfo;
  onClose: () => void;
}

export const UpdateModal: React.FC<UpdateModalProps> = ({
  updateInfo,
  onClose,
}) => {
  const [isDownloading, setIsDownloading] = useState(false);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [downloadError, setDownloadError] = useState<string | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    onUpdateProgress((p) => {
      setProgress(p);
      if (p.status === "error") {
        setIsDownloading(false);
        setDownloadError(p.errorMessage || "Download encountered an error");
      }
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(console.warn);

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const handleStartUpdate = async () => {
    if (!updateInfo.downloadUrl) {
      window.open(updateInfo.releasePageUrl, "_blank");
      return;
    }

    try {
      setIsDownloading(true);
      setDownloadError(null);
      await downloadAndInstallUpdate(updateInfo.downloadUrl);
    } catch (err: any) {
      setIsDownloading(false);
      setDownloadError(
        typeof err === "string" ? err : err?.message || "Failed to download update"
      );
    }
  };

  const formatBytes = (bytes: number) => {
    if (!bytes || bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/60 backdrop-blur-xs p-4 animate-in fade-in duration-150">
      <div className="bg-white rounded-2xl shadow-2xl border border-slate-200 max-w-lg w-full overflow-hidden flex flex-col max-h-[90vh]">
        {/* Header */}
        <div className="bg-gradient-to-r from-blue-600 via-indigo-600 to-blue-700 px-6 py-5 text-white flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-white/15 backdrop-blur-md flex items-center justify-center">
              <Sparkles className="w-5 h-5 text-amber-300" />
            </div>
            <div>
              <h2 className="text-base font-bold tracking-tight">
                Software Update Available
              </h2>
              <p className="text-xs text-blue-100 font-medium">
                SyncDrop {updateInfo.latestVersion} is ready to install
              </p>
            </div>
          </div>
          {!isDownloading && (
            <button
              onClick={onClose}
              className="text-white/70 hover:text-white p-1 rounded-lg hover:bg-white/10 transition"
              title="Close"
            >
              <X className="w-5 h-5" />
            </button>
          )}
        </div>

        {/* Content Body */}
        <div className="p-6 overflow-y-auto space-y-4 text-xs">
          {/* Version Comparison Card */}
          <div className="flex items-center justify-between p-3.5 bg-slate-50 border border-slate-200/80 rounded-xl">
            <div>
              <span className="text-slate-500 block text-[11px] font-medium">
                Current Installed
              </span>
              <span className="font-mono text-sm font-semibold text-slate-700">
                v{updateInfo.currentVersion}
              </span>
            </div>
            <div className="flex items-center gap-2 text-blue-600">
              <ArrowRight className="w-4 h-4" />
            </div>
            <div className="text-right">
              <span className="text-slate-500 block text-[11px] font-medium">
                Latest Available
              </span>
              <span className="font-mono text-sm font-bold text-emerald-600">
                v{updateInfo.latestVersion}
              </span>
            </div>
          </div>

          {/* Release Notes */}
          <div>
            <div className="flex items-center justify-between mb-1.5">
              <span className="font-semibold text-slate-800 text-xs">
                What's New in {updateInfo.releaseName}:
              </span>
              {updateInfo.fileSize && (
                <span className="text-[11px] text-slate-500 font-mono">
                  Size: {formatBytes(updateInfo.fileSize)}
                </span>
              )}
            </div>
            <div className="bg-slate-50 border border-slate-200 rounded-xl p-3 text-slate-600 font-mono text-[11px] max-h-40 overflow-y-auto whitespace-pre-wrap leading-relaxed">
              {updateInfo.releaseNotes.trim() ||
                "Performance optimizations, audio synchronization improvements, and bug fixes."}
            </div>
          </div>

          {/* Download Progress Bar */}
          {isDownloading && progress && (
            <div className="bg-blue-50/70 border border-blue-200/80 rounded-xl p-4 space-y-2">
              <div className="flex items-center justify-between text-xs font-medium text-slate-700">
                <span>
                  {progress.status === "installing"
                    ? "Launching Installer..."
                    : progress.status === "completed"
                    ? "Update Ready!"
                    : "Downloading update..."}
                </span>
                <span className="font-mono font-bold text-blue-700">
                  {Math.round(progress.percentage)}%
                </span>
              </div>
              <div className="w-full bg-slate-200 rounded-full h-2.5 overflow-hidden">
                <div
                  className="bg-blue-600 h-2.5 rounded-full transition-all duration-150"
                  style={{ width: `${progress.percentage}%` }}
                />
              </div>
              <div className="flex items-center justify-between text-[11px] text-slate-500 font-mono">
                <span>{formatBytes(progress.bytesDownloaded)}</span>
                <span>
                  {progress.totalBytes > 0
                    ? formatBytes(progress.totalBytes)
                    : "Calculating size..."}
                </span>
              </div>
            </div>
          )}

          {/* Error Message */}
          {downloadError && (
            <div className="flex items-start gap-2.5 p-3 rounded-xl bg-red-50 border border-red-200 text-red-700 text-xs">
              <AlertCircle className="w-4 h-4 flex-shrink-0 mt-0.5" />
              <div className="flex-1">
                <p className="font-semibold">Update download failed</p>
                <p className="text-[11px] mt-0.5">{downloadError}</p>
              </div>
            </div>
          )}
        </div>

        {/* Footer Actions */}
        <div className="bg-slate-50 px-6 py-4 border-t border-slate-200 flex items-center justify-between gap-3">
          <a
            href={updateInfo.releasePageUrl}
            target="_blank"
            rel="noreferrer"
            className="text-xs text-slate-500 hover:text-blue-600 flex items-center gap-1.5 font-medium transition"
          >
            <span>View on GitHub</span>
            <ExternalLink className="w-3.5 h-3.5" />
          </a>

          <div className="flex items-center gap-2">
            {!isDownloading && (
              <button
                onClick={onClose}
                className="px-3 py-1.5 text-xs text-slate-600 hover:text-slate-800 hover:bg-slate-200/60 rounded-lg transition font-medium"
              >
                Later
              </button>
            )}

            {isDownloading && progress?.status === "completed" ? (
              <div className="flex items-center gap-1.5 px-4 py-2 bg-emerald-600 text-white rounded-xl text-xs font-semibold shadow-xs">
                <CheckCircle className="w-4 h-4" />
                <span>Installer Launched</span>
              </div>
            ) : isDownloading ? (
              <button
                disabled
                className="flex items-center gap-2 px-4 py-2 bg-blue-400 text-white rounded-xl text-xs font-semibold cursor-not-allowed shadow-xs"
              >
                <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                <span>Downloading...</span>
              </button>
            ) : (
              <button
                onClick={handleStartUpdate}
                className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-xl text-xs font-semibold shadow-md shadow-blue-500/20 transition active:scale-95"
              >
                <Download className="w-4 h-4" />
                <span>Download &amp; Install Now</span>
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
