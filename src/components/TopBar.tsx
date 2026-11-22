import React, { useState } from "react";
import {
  Wifi,
  Film,
  Copy,
  Check,
  User,
  Settings,
  Bell,
  Minus,
  Square,
  X,
  Share2,
  Activity,
} from "lucide-react";
import type { ActiveTab } from "../types";

interface TopBarProps {
  activeTab: ActiveTab;
  onTabChange: (tab: ActiveTab) => void;
  nickname: string;
  onNicknameChange: (name: string) => void;
  localIp: string;
  onlinePeerCount: number;
  onSimulateIncoming?: () => void;
  onOpenSettings?: () => void;
  isDemoMode?: boolean;
}

export const TopBar: React.FC<TopBarProps> = ({
  activeTab,
  onTabChange,
  nickname,
  onNicknameChange,
  localIp,
  onlinePeerCount,
  onSimulateIncoming,
  onOpenSettings,
  isDemoMode = false,
}) => {
  const [isEditingNickname, setIsEditingNickname] = useState(false);
  const [tempNickname, setTempNickname] = useState(nickname);
  const [copiedIp, setCopiedIp] = useState(false);

  const handleCopyIp = () => {
    navigator.clipboard.writeText(localIp);
    setCopiedIp(true);
    setTimeout(() => setCopiedIp(false), 2000);
  };

  const handleSaveNickname = () => {
    if (tempNickname.trim()) {
      onNicknameChange(tempNickname.trim());
    }
    setIsEditingNickname(false);
  };

  return (
    <header className="bg-white border-b border-slate-200 select-none flex-shrink-0 z-30">
      {/* OS-style Header Strip */}
      <div className="flex items-center justify-between px-4 py-1.5 bg-slate-100/90 text-xs text-slate-500 border-b border-slate-200/80">
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
          <span className="font-semibold text-slate-700 tracking-tight">LOCAL SERVICE ACTIVE</span>
          <span className="text-slate-300">|</span>
          <span className="text-slate-500 font-mono text-[11px]">v0.1.0-win64</span>
          {isDemoMode && (
            <span className="ml-2 text-[10px] font-semibold bg-amber-100 text-amber-800 px-1.5 py-0.2 rounded border border-amber-300">
              Demo Preview Mode
            </span>
          )}
        </div>
        <div className="text-slate-600 font-medium text-xs hidden sm:block">
          SyncDrop for Windows Desktop
        </div>
        <div className="flex items-center gap-3">
          <button
            className="text-slate-400 hover:text-slate-700 hover:bg-slate-200/60 p-1 rounded transition"
            title="Minimize"
          >
            <Minus className="w-3.5 h-3.5" />
          </button>
          <button
            className="text-slate-400 hover:text-slate-700 hover:bg-slate-200/60 p-1 rounded transition"
            title="Maximize"
          >
            <Square className="w-3 h-3" />
          </button>
          <button
            className="text-slate-400 hover:text-white hover:bg-red-500 p-1 rounded transition"
            title="Close"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Main Application Toolbar */}
      <div className="px-6 py-3 flex items-center justify-between gap-4 flex-wrap">
        {/* Left: Branding & Tagline */}
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-blue-600 via-blue-500 to-indigo-600 flex items-center justify-center text-white shadow-md shadow-blue-500/20 flex-shrink-0">
            <Share2 className="w-5 h-5 stroke-[2.2]" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <span className="text-lg font-bold text-slate-900 tracking-tight">SyncDrop</span>
              <span className="text-[10px] font-bold uppercase tracking-wider px-2 py-0.5 rounded-full bg-blue-50 text-blue-700 border border-blue-200">
                LAN ONLY
              </span>
            </div>
            <p className="text-xs text-slate-500 font-medium">Peer-to-Peer Gigabit Direct &amp; Clock Lock</p>
          </div>
        </div>

        {/* Center: Nickname Editor */}
        <div className="flex items-center bg-slate-50 rounded-xl border border-slate-200 px-3 py-1.5 text-xs gap-2 shadow-2xs">
          <User className="w-4 h-4 text-blue-600 flex-shrink-0" />
          <span className="text-slate-500 font-medium">Nickname:</span>
          {isEditingNickname ? (
            <div className="flex items-center gap-1.5">
              <input
                type="text"
                value={tempNickname}
                onChange={(e) => setTempNickname(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleSaveNickname()}
                className="bg-white px-2 py-0.5 text-xs rounded border border-blue-300 focus:outline-none focus:ring-2 focus:ring-blue-500/30 font-semibold text-slate-800"
                autoFocus
              />
              <button
                onClick={handleSaveNickname}
                className="text-emerald-600 hover:text-emerald-700 p-0.5 transition"
                title="Save Nickname"
              >
                <Check className="w-4 h-4" />
              </button>
            </div>
          ) : (
            <button
              onClick={() => {
                setTempNickname(nickname);
                setIsEditingNickname(true);
              }}
              className="font-semibold text-slate-800 hover:text-blue-600 flex items-center gap-1.5 transition group"
              title="Click to edit nickname"
            >
              <span>{nickname}</span>
              <span className="text-[10px] text-slate-400 group-hover:text-blue-500 underline">edit</span>
            </button>
          )}
        </div>

        {/* Right: Primary Navigation Tabs */}
        <div className="flex items-center gap-2">
          <nav className="flex items-center bg-slate-100 p-1 rounded-xl border border-slate-200">
            <button
              onClick={() => onTabChange("wifi-send")}
              className={`flex items-center gap-2 px-4 py-2 rounded-lg text-xs font-semibold transition-all duration-150 ${
                activeTab === "wifi-send"
                  ? "bg-white text-blue-700 shadow-xs border border-slate-200/50"
                  : "text-slate-600 hover:text-slate-900 hover:bg-slate-200/50"
              }`}
            >
              <Wifi className="w-3.5 h-3.5" />
              <span>Wi-Fi Send</span>
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
            </button>
            <button
              onClick={() => onTabChange("watch-party")}
              className={`flex items-center gap-2 px-4 py-2 rounded-lg text-xs font-semibold transition-all duration-150 ${
                activeTab === "watch-party"
                  ? "bg-white text-blue-700 shadow-xs border border-slate-200/50"
                  : "text-slate-600 hover:text-slate-900 hover:bg-slate-200/50"
              }`}
            >
              <Film className="w-3.5 h-3.5" />
              <span>Watch Party</span>
              <span className="text-[9px] bg-amber-100 text-amber-900 px-1.5 py-0.2 rounded font-bold uppercase tracking-wider">
                SYNC
              </span>
            </button>
          </nav>
        </div>
      </div>

      {/* Sub-bar / Network Status Bar */}
      <div className="px-6 py-2 bg-slate-50 border-t border-slate-200/70 flex items-center justify-between text-xs flex-wrap gap-2">
        <div className="flex items-center gap-3">
          {/* IP Chip */}
          <div className="flex items-center gap-1.5 bg-white border border-slate-200 px-2.5 py-1 rounded-lg text-slate-700 font-mono text-[11px] shadow-2xs">
            <Activity className="w-3 h-3 text-blue-600" />
            <span>{localIp}</span>
            <button
              onClick={handleCopyIp}
              className="text-slate-400 hover:text-slate-700 p-0.5 rounded transition"
              title="Copy IP Address"
            >
              {copiedIp ? <Check className="w-3 h-3 text-emerald-600" /> : <Copy className="w-3 h-3" />}
            </button>
          </div>

          {/* Online Friends Chip */}
          <div className="flex items-center gap-1.5 bg-emerald-50 border border-emerald-200 px-2.5 py-1 rounded-lg text-emerald-800 font-medium text-[11px]">
            <span className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
            <span>{onlinePeerCount} friends online</span>
          </div>

          {/* Test Trigger */}
          {onSimulateIncoming && (
            <button
              onClick={onSimulateIncoming}
              className="flex items-center gap-1.5 bg-white hover:bg-blue-50 border border-slate-200 hover:border-blue-300 px-2.5 py-1 rounded-lg text-slate-600 hover:text-blue-700 text-[11px] font-medium transition shadow-2xs"
            >
              <Bell className="w-3.5 h-3.5 text-blue-600" />
              <span>Simulate Incoming</span>
            </button>
          )}
        </div>

        {/* Right Info & Settings */}
        <div className="flex items-center gap-4 text-slate-500 text-[11px]">
          <span className="hidden md:inline font-mono text-[10px]">UDP Discovery: 224.0.0.167:53317</span>
          <span className="hidden md:inline font-mono text-[10px]">WS Sync: :53318</span>
          <button
            onClick={onOpenSettings}
            className="text-slate-400 hover:text-slate-700 hover:bg-slate-200/60 p-1.5 rounded-lg transition"
            title="Settings"
          >
            <Settings className="w-4 h-4" />
          </button>
        </div>
      </div>
    </header>
  );
};
