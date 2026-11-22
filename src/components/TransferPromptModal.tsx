import React from "react";
import { DownloadCloud, Check, X, Laptop, AlertCircle, HardDrive } from "lucide-react";
import type { IncomingTransferPrompt } from "../types";

interface TransferPromptModalProps {
  prompt: IncomingTransferPrompt;
  onAccept: (transferId: string) => void;
  onDecline: (transferId: string) => void;
}

export const TransferPromptModal: React.FC<TransferPromptModalProps> = ({
  prompt,
  onAccept,
  onDecline,
}) => {
  const formatFileSize = (bytes: number): string => {
    if (bytes >= 1024 * 1024 * 1024) return (bytes / (1024 * 1024 * 1024)).toFixed(2) + " GB";
    if (bytes >= 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + " MB";
    if (bytes >= 1024) return (bytes / 1024).toFixed(1) + " KB";
    return bytes + " B";
  };

  return (
    <div className="fixed inset-0 z-50 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 select-none">
      <div className="bg-white rounded-2xl border border-slate-200 shadow-2xl max-w-md w-full p-6 flex flex-col gap-5 animate-in fade-in zoom-in-95 duration-150">
        {/* Header Icon & Title */}
        <div className="flex items-center gap-3.5">
          <div className="w-12 h-12 rounded-xl bg-blue-50 text-blue-600 flex items-center justify-center shadow-inner flex-shrink-0">
            <DownloadCloud className="w-6 h-6 stroke-[2]" />
          </div>
          <div>
            <h3 className="text-base font-bold text-slate-900 tracking-tight">
              Incoming File Transfer
            </h3>
            <p className="text-xs text-slate-500">
              A friend on your local Wi-Fi wants to send you a file.
            </p>
          </div>
        </div>

        {/* Sender & File Card */}
        <div className="bg-slate-50 border border-slate-200/90 rounded-xl p-4 flex flex-col gap-3">
          {/* Sender details */}
          <div className="flex items-center justify-between text-xs pb-3 border-b border-slate-200">
            <div className="flex items-center gap-2 min-w-0">
              <Laptop className="w-4 h-4 text-blue-600 flex-shrink-0" />
              <div className="truncate">
                <span className="font-semibold text-slate-800">{prompt.senderAlias}</span>
                <span className="text-[11px] text-slate-400 ml-1.5 hidden sm:inline">
                  ({prompt.senderDeviceModel})
                </span>
              </div>
            </div>
            <span className="font-mono text-[11px] text-slate-500 font-medium">
              {prompt.senderIp}
            </span>
          </div>

          {/* File details */}
          <div className="flex flex-col gap-1">
            <span className="text-[10px] text-slate-400 uppercase tracking-wider font-bold">
              File Details
            </span>
            <div className="flex items-start gap-2 mt-0.5">
              <HardDrive className="w-4 h-4 text-slate-400 flex-shrink-0 mt-0.5" />
              <div className="min-w-0">
                <p className="text-xs font-bold text-slate-900 break-all">{prompt.file.filename}</p>
                <span className="text-xs font-mono text-blue-600 font-semibold">
                  {formatFileSize(prompt.file.size)}
                </span>
              </div>
            </div>
          </div>
        </div>

        {/* Safety Note */}
        <div className="flex items-start gap-2.5 text-[11px] text-slate-600 bg-blue-50/60 border border-blue-200/70 rounded-xl p-3">
          <AlertCircle className="w-4 h-4 text-blue-600 flex-shrink-0 mt-0.5" />
          <span>
            Transfers stream peer-to-peer across your private LAN and will be saved directly into
            your system Downloads folder.
          </span>
        </div>

        {/* Action Buttons */}
        <div className="grid grid-cols-2 gap-3 pt-1">
          <button
            onClick={() => onDecline(prompt.transferId)}
            className="flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl border border-slate-200 hover:bg-slate-100 text-slate-700 text-xs font-semibold transition active:scale-95 shadow-2xs"
          >
            <X className="w-4 h-4 text-slate-500" />
            <span>Decline</span>
          </button>
          <button
            onClick={() => onAccept(prompt.transferId)}
            className="flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl bg-blue-600 hover:bg-blue-700 text-white text-xs font-semibold shadow-md shadow-blue-500/20 transition active:scale-95"
          >
            <Check className="w-4 h-4 stroke-[2.5]" />
            <span>Accept File</span>
          </button>
        </div>
      </div>
    </div>
  );
};
