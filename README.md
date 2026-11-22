# SyncDrop 🎬⚡

**Offline, Zero-Cloud Local Wi-Fi File Sharing & PTP-Synchronized Watch Party for Windows**

SyncDrop is a modern Windows desktop application that combines high-speed peer-to-peer Wi-Fi file transfers with millisecond-accurate synchronized movie playback. Designed from the ground up to be 100% offline, privacy-first, and zero-cloud.

---

## 🚀 Key Features

### 1. 📡 Zero-Config Local Wi-Fi Discovery
- **UDP Multicast & Subnet Broadcast** (Port 53317) automatically discovers other devices running SyncDrop on your Wi-Fi subnet within 1 second.
- Automatic avatar and device identification (desktop, laptop, tablet, mobile).
- Dynamic heartbeat with stale peer detection (15-second timeout).
- Instant watch-room presence announcements across LAN.

### 2. ⚡ High-Speed Direct P2P File Transfers
- Direct TCP socket streaming on Port 53317 (no cloud servers, no intermediaries).
- Transfers gigabyte-sized video and media files at full Wi-Fi speeds (30–90+ MB/s).
- **Chunked Streaming & Flow Control**: 64 KB memory chunks ensure minimal RAM usage even for 50GB+ 4K movies.
- **Explicit Consent**: Interactive Accept / Decline prompt before any transfer starts.
- **Security & Integrity**: End-to-end async SHA-256 verification, filename sanitization, and path-traversal prevention (`C:\Users\<User>\Downloads\SyncDrop`).
- **Multi-Recipient Transfers**: Send files to multiple peers simultaneously with independent progress tracking.

### 3. 🍿 Synchronized Movie Watch Party (mpv Engine)
- Integrated high-performance **mpv** sidecar media player controlled via bidirectional JSON IPC.
- Authoritative host model over local WebSocket connections (Port 53318).
- **Real-Time Synchronization**: Synchronizes Play, Pause, Timeline Seek, and Playback Speed.
- **Adaptive Drift Correction**:
  - *Drift < 0.3s*: Seamless playback, no correction applied.
  - *Drift 0.3s – 1.5s*: Gentle speed micro-adjustments (0.95x – 1.05x) until aligned.
  - *Drift > 1.5s*: Instant hard seek to host's projected playback timestamp.
- **Network Latency Compensation**: Calculates host's expected playback position taking one-way network transit latency into account.
- **Late-Join Catch Up**: New participants joining mid-movie automatically jump to the host's current position (e.g. catches up to 32 minutes while loading).
- **Granular Host Permissions**: The host can choose exactly which controls friends can access:
  - Play / Pause
  - Rewind (-10s)
  - Forward (+10s)
  - Timeline Seek
  - Speed Control
  - "All Controls" (Full Access) or Strict Host-Only
- **Movie Compatibility Verification**: Automatic duration, format, and async SHA-256 hash checks to prevent mismatched movie files from desyncing.
- **Missing-Movie Workflow**: If a participant doesn't have the movie, the host can send it directly over LAN with one click before joining.

---

## 📥 Installation

### Latest Windows Installer (NSIS .exe)
You can directly run the pre-built installer located in this repository:
📁 **[`release/SyncDrop_0.1.0_x64-setup.exe`](release/SyncDrop_0.1.0_x64-setup.exe)**

1. Double-click **`SyncDrop_0.1.0_x64-setup.exe`**.
2. Review and accept the **Offline Privacy Policy & License Agreement**.
3. Choose installation options:
   - Select installation folder (`C:\Program Files\SyncDrop` or local user directory).
   - Check **Create a Desktop Shortcut**.
   - Check **Create a Start Menu Shortcut**.
4. Click **Install** and launch SyncDrop!

> [!IMPORTANT]
> **Windows Firewall Prompt**: When launched for the first time, Windows Defender Firewall will ask for permission to communicate on **Private networks**. Check "Private networks" and click **Allow access**. This allows SyncDrop to discover devices and stream media over your Wi-Fi.

---

## 🛠️ Tech Stack & Architecture

- **Frontend**: React 19, TypeScript, Tailwind CSS v4, Lucide React, Vite.
- **Backend**: Rust, Tauri v2, Tokio async runtime, Axum, Tungstenite (WebSockets), Socket2 (UDP multicast).
- **Media Engine**: mpv (bundled Windows x86_64 sidecar binary with JSON IPC control).
- **Packaging**: Tauri v2 NSIS Windows Installer with embedded sidecar.

---

## 💻 Building from Source

### Prerequisites
- [Node.js 18+](https://nodejs.org/)
- [Rust 1.77+](https://rustup.rs/) (with `x86_64-pc-windows-msvc` toolchain)
- Visual Studio C++ Build Tools (with Windows 10/11 SDK)

### Step-by-Step Build

1. **Clone the repository:**
   ```bash
   git clone https://github.com/trishulreddy3/syncdrop.git
   cd syncdrop
   ```

2. **Install frontend dependencies:**
   ```bash
   npm install
   ```

3. **Run in development mode:**
   ```bash
   npm run tauri dev
   ```

4. **Run the automated test suite:**
   ```bash
   cargo test --manifest-path src-tauri/Cargo.toml
   ```

5. **Build the production NSIS installer:**
   ```bash
   npm run tauri build
   ```
   The generated installer will be located at:
   `src-tauri/target/release/bundle/nsis/SyncDrop_0.1.0_x64-setup.exe`

---

## 🔒 Offline Privacy Guarantee

SyncDrop contains **zero telemetry**, **zero cloud tracking**, and **zero external analytics**. All communication is restricted to your local area network (LAN) using private IPv4 addresses (`192.168.x.x` / `10.x.x.x`). No internet connection is ever required.

See [LICENSE.md](LICENSE.md) for full license terms and offline privacy policy.
