# SyncDrop --- Project Specification

**File:** `PROJECT_SPEC.md`\
**Project:** SyncDrop\
**Document purpose:** Authoritative product, architecture,
implementation, and acceptance specification for AI-assisted
development.\
**Initial release target:** Windows 10/11, x86_64\
**Primary use case:** Offline file sharing and synchronized movie
playback for approximately 10 friends on the same local Wi-Fi network.

> **Instruction for future AI work:** Treat this document as the source
> of truth for SyncDrop. Before proposing architecture changes or
> writing code, read this specification. Preserve the stated
> requirements and technology choices. Clearly distinguish implemented
> features from planned features. Do not silently replace Tauri, Rust,
> React, TypeScript, Tailwind CSS, mpv, the LAN-only design, or the
> specified network behavior. If a requirement conflicts with a platform
> limitation or a security concern, explain the issue and propose the
> smallest compatible solution.

------------------------------------------------------------------------

## 1. Product Goal and User Scenario

Build **SyncDrop**, a desktop application that combines two
capabilities:

1.  **LAN file sharing**, similar in user experience to LocalSend.
2.  **Synchronized movie playback**, similar in user experience to
    Syncplay.

The application is intended for a group of approximately 10 friends
connected to the **same local Wi-Fi network**. It must work without
cloud services, accounts, external servers, or an internet connection.

### 1.1 LAN File Sharing

Users should be able to:

-   Automatically discover other online SyncDrop users on the same
    Wi-Fi/LAN.
-   See each peer's nickname, device information, IP address, and online
    status.
-   Select one or multiple recipients.
-   Choose files using a file picker or drag-and-drop.
-   Send small or very large files, including movie files.
-   Show an incoming transfer request to each recipient.
-   Allow each recipient to **Accept** or **Decline** independently.
-   Transfer accepted files directly over the LAN.
-   Display real-time transfer progress, speed, transferred bytes, total
    bytes, and status.
-   Send to multiple recipients without requiring a cloud relay.
-   Show active, completed, declined, cancelled, and failed transfers.

### 1.2 Synchronized Movie Playback

Users should be able to:

-   Host a local watch room from one laptop.
-   Join a room hosted by another SyncDrop user.
-   Discover available hosts or join by entering the host's IP address.
-   Select a local movie file on each participant's laptop.
-   Synchronize playback across connected laptops.
-   Synchronize **Play, Pause, Seek, and playback speed**.
-   Detect playback drift and correct it smoothly.
-   Display room members and their synchronization status.
-   Use an embedded/bundled `mpv` media engine, rather than requiring
    users to install VLC or another external player.

Each participant is expected to have a local copy of the movie. SyncDrop
synchronizes playback state; it does not need to stream the movie itself
as part of the watch-room feature. Users can transfer the movie first
through the Wi-Fi Send feature if necessary.

------------------------------------------------------------------------

## 2. Technology Stack and Packaging Rules

### 2.1 Required Stack

  Layer                        Required technology
  ---------------------------- ---------------------------------------
  Desktop shell                Tauri v2
  Native/backend layer         Rust
  Frontend framework           React
  Frontend language            TypeScript
  Styling                      Tailwind CSS
  Media engine                 Bundled `mpv` sidecar
  Media control                mpv JSON IPC
  Peer discovery               UDP multicast with broadcast fallback
  File transfer                Direct HTTP over TCP
  Watch-room synchronization   WebSocket
  Installer                    Tauri NSIS installer
  Initial target               Windows 10/11, x86_64

### 2.2 Offline and LAN-Only Requirement

-   The application must work fully offline after installation.
-   No cloud backend, hosted session server, user account, or internet
    connection may be required.
-   Discovery, file transfers, and watch-room synchronization must
    remain inside the local network.
-   Do not add analytics, telemetry, cloud storage, or third-party relay
    services.
-   Build-time dependency downloads are acceptable during development,
    but normal application operation must not depend on internet access.

### 2.3 Tauri and mpv Sidecar

Bundle the Windows mpv executable at:

``` text
src-tauri/binaries/mpv-x86_64-pc-windows-msvc.exe
```

Declare the sidecar in `src-tauri/tauri.conf.json` under
`bundle.externalBin` using the Tauri v2-compatible configuration:

``` json
{
  "bundle": {
    "externalBin": [
      "binaries/mpv"
    ]
  }
}
```

The actual executable must use Tauri's expected target-triple suffix.
The application must launch and control mpv through its JSON IPC
interface, using `--input-ipc-server` with a suitable local IPC
endpoint.

Implementation requirements:

-   Verify that the bundled mpv executable exists before attempting to
    launch it.
-   Resolve the packaged sidecar path using Tauri's supported sidecar
    APIs.
-   Do not hardcode a development-only absolute path.
-   Start mpv as a managed child process.
-   Keep track of its process lifecycle.
-   Connect to its JSON IPC endpoint.
-   Send commands and read property updates.
-   Handle mpv startup failure, IPC disconnection, unexpected process
    exit, and application shutdown.
-   Stop or clean up the managed mpv process when appropriate.
-   Do not assume the mpv IPC endpoint is a network-accessible service.
    Keep it local to the device.

**Important:** Confirm the exact Tauri v2 sidecar configuration and
permissions against the Tauri version used by the project. Do not copy
Tauri v1 configuration blindly.

### 2.4 Build and Installer

The Windows release must be buildable with:

``` bash
npm run tauri build
```

The project must be configured to generate a Windows NSIS installer
(`.exe`) for x86_64.

Document all required prerequisites, including:

-   Node.js and npm.
-   Rust toolchain.
-   Tauri v2 prerequisites for Windows.
-   WebView2 runtime availability/installation behavior.
-   The correct mpv Windows x86_64 binary.
-   Any required Windows SDK or build tools.

The final installer must include the application and the mpv sidecar.
Users should not need to install Rust, Node.js, or mpv to run the
packaged application.

------------------------------------------------------------------------

## 3. Network Architecture and Protocols

The application uses direct peer-to-peer communication on the local
network. The initial design uses the following ports:

  ------------------------------------------------------------------------
  Purpose               Protocol                                      Port
  --------------------- --------------------- ----------------------------
  Peer discovery        UDP                                          53317
                        multicast/broadcast   

  HTTP file transfer    HTTP over TCP                 53317 or a dedicated
                                                             transfer port

  Watch-room            WebSocket over TCP                           53318
  synchronization                             
  ------------------------------------------------------------------------

If discovery and HTTP transfer cannot safely or reliably share port
`53317` on the target operating systems, use a dedicated transfer port.
Keep the selected ports visible in the project documentation and
consistent throughout the code.

### 3.1 Peer Discovery

Use:

-   UDP multicast address: `224.0.0.167`
-   UDP port: `53317`
-   UDP broadcast fallback on local subnets when multicast is blocked or
    unavailable.

Discovery must:

1.  Announce the local device periodically.
2.  Listen for announcements from other SyncDrop instances.
3.  Add or update peers when announcements arrive.
4.  Mark peers stale/offline if they have not been seen for more than
    **15 seconds**.
5.  Remove or expire stale peers according to a documented cleanup
    policy.
6.  Avoid showing duplicate entries for the same device.
7.  Handle network-interface changes and temporary Wi-Fi disconnections.

The discovery announcement must contain:

  -----------------------------------------------------------------------
  Field                               Description
  ----------------------------------- -----------------------------------
  `id`                                Stable identifier for this SyncDrop
                                      installation/device

  `alias`                             User's editable nickname

  `deviceModel`                       Device model or a reasonable
                                      platform-provided description

  `deviceType`                        `"desktop"`

  `port`                              Discovery/service port, initially
                                      `53317`

  `protocol`                          `"http"`
  -----------------------------------------------------------------------

Use a documented JSON message format. Validate received messages before
trusting their fields. Do not treat a nickname or device identifier as
authentication.

#### Discovery fallback

If multicast does not work on a router, hotspot, or Windows network
profile:

-   Attempt UDP broadcast on suitable local subnet broadcast addresses.
-   Handle multiple network interfaces carefully.
-   Avoid sending discovery packets to public or unrelated networks.
-   Explain that some guest Wi-Fi networks enable client isolation,
    which can prevent peer-to-peer communication even when devices
    appear connected to the same Wi-Fi.

### 3.2 Direct File Transfer

Use direct HTTP over TCP between sender and recipient devices.

The transfer workflow is:

1.  Sender selects one or more files.
2.  Sender selects one or more discovered recipients.
3.  Sender sends a **Prepare Upload** request containing transfer
    metadata.
4.  Each recipient receives an incoming-transfer prompt.
5.  Each recipient independently chooses **Accept** or **Decline**.
6.  The sender transfers file bytes only to recipients who accepted.
7.  Both sides receive progress updates.
8.  The transfer is marked completed, declined, cancelled, or failed.

The metadata must include at least:

-   `fileId`: unique transfer/file identifier.
-   `filename`: original filename, safely handled.
-   `size`: expected file size in bytes.
-   A sender/device identifier.
-   Any additional transfer/session identifier needed to correlate
    events.

The upload should stream bytes in chunks rather than loading the entire
file into memory. It must support large files.

Transfer progress events should include:

  Field          Meaning
  -------------- ------------------------
  `bytesSent`    Bytes sent so far
  `totalBytes`   Total expected bytes
  `speed`        Current transfer speed
  `status`       Current transfer state

The receiver UI must show progress based on bytes actually received, not
only bytes submitted by the sender.

#### Multi-recipient behavior

-   Each recipient has an independent Accept/Decline decision.
-   A decline or failure for one recipient must not cancel other
    recipients' transfers.
-   Track progress independently for every recipient.
-   The sender must be able to see which recipients accepted, declined,
    completed, or failed.
-   Avoid reading a large file fully into memory for every recipient.
-   Document whether uploads are sent concurrently or sequentially and
    enforce a sensible concurrency limit.

#### Transfer safety and reliability

-   Sanitize filenames and prevent path traversal.
-   Never allow a received filename to choose an arbitrary destination
    path.
-   Let the user choose or configure a download destination.
-   Do not overwrite an existing file silently.
-   Validate the received byte count against the expected size.
-   Handle interrupted connections and partial files.
-   Clean up incomplete files safely, or retain them only if resumable
    transfers are deliberately implemented.
-   Provide clear error messages for disk-full, permission, network, and
    connection errors.
-   Do not automatically execute received files.
-   Bind transfer services to appropriate local interfaces and do not
    expose them to the public internet.
-   Consider a per-transfer acceptance token or other
    request-correlation mechanism so unrelated LAN devices cannot
    trivially submit data into an accepted transfer.

### 3.3 Synchronized Watch Room

The room creator hosts a local WebSocket server on:

``` text
Port: 53318
```

Clients connect to:

``` text
ws://<host-ip>:53318
```

The room host is authoritative for playback state. Participants send
user actions to the host; the host validates and broadcasts the
resulting state to room members.

Required message types:

  Message        Purpose
  -------------- -----------------------------------------
  `JOIN`         A participant requests to join a room
  `LEAVE`        A participant leaves the room
  `STATE_SYNC`   Broadcasts authoritative playback state
  `SEEK`         Requests or announces a seek
  `PLAY`         Requests or announces playback
  `PAUSE`        Requests or announces pause
  `FILE_INFO`    Shares movie identity information

A `STATE_SYNC` message must include:

-   `position`: playback position in seconds.
-   `paused`: whether playback is paused.
-   `speed`: playback speed.
-   `timestamp`: time at which the state was generated.
-   `actor`: device/user responsible for the action.

Use a documented JSON schema for every message. Include room/session
identifiers and message correlation fields where useful. Validate
message types and values before acting on them.

#### Room behavior

-   A host can create a room.
-   A participant can join using a discovered host or manually entered
    host IP.
-   The host can see connected participants.
-   Participants can leave cleanly.
-   The UI must show connection and synchronization status.
-   A host disconnect should produce a clear room-ended/disconnected
    state.
-   Reconnection behavior should be deliberate and documented; do not
    silently create a different room.

#### Movie matching

`FILE_INFO` must include at least:

-   Filename.
-   File size.

Filename and size are a basic compatibility check, not proof that two
files contain identical content. If stronger verification is needed, add
a hash (for example SHA-256) as an optional or subsequent enhancement.
Do not hash very large files on the UI thread.

When a participant has not loaded a matching local movie, show a clear
instruction to select the correct local file. Do not automatically play
a different file based only on a filename match.

### 3.4 Playback Drift Correction

The host periodically broadcasts the current playback state. Each client
compares its local mpv `time-pos` with the host's expected position.

Initial drift policy:

         Absolute drift Behavior
  --------------------- -----------------------------------------
       Less than `0.3s` No correction required
       `0.3s` to `1.5s` Gently adjust playback speed
    Greater than `1.5s` Seek/snap to the expected host position

For minor drift while playing:

-   Adjust mpv speed within `0.95x` to `1.05x`.
-   Return speed toward the room's normal playback speed after drift is
    corrected.
-   Never let temporary drift correction permanently change the user's
    selected room playback speed.

For large drift:

-   Seek to the host's expected position.
-   Avoid repeated seek loops caused by stale messages.
-   Account for message/network delay when estimating the host's current
    position.

The synchronization implementation must distinguish the room's intended
playback speed from a temporary speed adjustment used to correct drift.

#### Timing and synchronization notes

-   Use timestamps consistently and document their units.
-   Estimate expected host position using the most recent state, elapsed
    time, pause state, and playback speed.
-   Avoid applying old state messages after newer state messages.
-   Use sequence numbers or equivalent ordering metadata to reject stale
    updates.
-   Periodically resynchronize even when no explicit Play/Pause/Seek
    action occurs.
-   Prevent feedback loops in which a client correction is mistaken for
    a new user action and rebroadcast as a host command.

------------------------------------------------------------------------

## 4. UI and User Experience

Use a modern, clean desktop interface built with React, TypeScript, and
Tailwind CSS.

### 4.1 Top Bar

The top bar must show:

-   Editable user nickname.
-   Local IPv4 address.
-   Number of active/discovered peers.
-   Quick network/application status indicator.

Nickname requirements:

-   Allow the user to edit their nickname.
-   Persist the nickname locally.
-   Use a sensible default nickname when none has been set.
-   Do not require account creation.

### 4.2 Navigation

The application has two primary tabs:

1.  **Wi-Fi Send**
2.  **Watch Party**

### 4.3 Wi-Fi Send Tab

Must contain:

-   Discovered peers list.
-   Peer search/filter.
-   Single- and multi-peer selection.
-   File picker.
-   Drag-and-drop file zone.
-   Selected-file list.
-   Send action.
-   Incoming Accept/Decline transfer prompts.
-   Active transfer list.
-   Completed/declined/failed transfer history or status list.
-   Per-recipient progress, speed, and status.

The peer list should clearly distinguish online and stale/offline
devices. Prevent sending to a peer that is no longer reachable without
displaying an appropriate error.

### 4.4 Incoming Transfer Prompt

The incoming transfer modal must show:

-   Sender nickname/device.
-   Filename.
-   File size.
-   Accept button.
-   Decline button.

The prompt should not automatically accept. The receiver must explicitly
choose.

### 4.5 Watch Party Tab

Must contain:

-   **Host Room** action.
-   **Join Room** action.
-   Discovered room hosts.
-   Manual host-IP entry.
-   Local movie file selector.
-   mpv launch/player status.
-   Play, Pause, and Seek controls.
-   Playback position/duration where available.
-   Playback speed display/control.
-   Connected member list.
-   Connection and sync status for each member.
-   Clear indication when a participant has not loaded a matching movie.

The UI must communicate whether mpv is stopped, starting, running, or
unavailable.

### 4.6 UI State and Events

-   Keep UI state consistent with the Rust backend.
-   Use Tauri events or commands deliberately and document their
    responsibilities.
-   Do not use frontend-only mock data as if it were real network state.
-   Handle empty, loading, disconnected, permission-denied, and error
    states.
-   Keep long-running network and file operations out of blocking UI
    paths.

------------------------------------------------------------------------

## 5. Project Folder and File Layout

Use this project structure as the initial organization:

``` text
syncdrop/
├── PROJECT_SPEC.md
├── package.json
├── index.html
├── src/
│   ├── App.tsx
│   ├── types/
│   │   └── index.ts
│   ├── components/
│   │   ├── TopBar.tsx
│   │   ├── WifiSendTab.tsx
│   │   ├── TransferPromptModal.tsx
│   │   └── WatchPartyTab.tsx
│   └── lib/
│       └── tauriEvents.ts
└── src-tauri/
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── capabilities/
    │   └── default.json
    ├── binaries/
    │   └── mpv-x86_64-pc-windows-msvc.exe
    └── src/
        ├── main.rs
        ├── lib.rs
        ├── discovery.rs
        ├── transfer.rs
        ├── sync_room.rs
        └── mpv_controller.rs
```

This is the baseline structure, not a restriction against adding
well-organized modules. Add modules for shared protocol types,
configuration, logging, validation, network interfaces, or tests when
useful. Keep responsibilities separated.

### 5.1 Suggested Rust Module Responsibilities

  -----------------------------------------------------------------------
  File                                Responsibility
  ----------------------------------- -----------------------------------
  `main.rs`                           Application entry point

  `lib.rs`                            Tauri builder, plugin setup,
                                      command registration, application
                                      state

  `discovery.rs`                      UDP multicast/broadcast discovery,
                                      peer tracking, stale-peer cleanup

  `transfer.rs`                       HTTP transfer server/client,
                                      request handling, streaming,
                                      progress

  `sync_room.rs`                      WebSocket room host/client, room
                                      messages, state distribution, drift
                                      coordination

  `mpv_controller.rs`                 mpv sidecar lifecycle, JSON IPC,
                                      playback properties and commands
  -----------------------------------------------------------------------

### 5.2 Suggested Frontend Responsibilities

  -----------------------------------------------------------------------
  File                                Responsibility
  ----------------------------------- -----------------------------------
  `App.tsx`                           Main application layout and tab
                                      navigation

  `types/index.ts`                    Shared frontend types for peers,
                                      transfers, rooms, and playback

  `TopBar.tsx`                        Nickname, local IP, peer count,
                                      status

  `WifiSendTab.tsx`                   Peer selection, file selection,
                                      outgoing/incoming transfer UI

  `TransferPromptModal.tsx`           Incoming transfer
                                      acceptance/decline

  `WatchPartyTab.tsx`                 Room controls, local movie
                                      selection, playback controls,
                                      members

  `lib/tauriEvents.ts`                Typed event subscriptions and
                                      frontend/backend event helpers
  -----------------------------------------------------------------------

------------------------------------------------------------------------

## 6. Implementation and Development Rules

Build the application incrementally. Each module must compile and be
testable before moving to the next major module.

Recommended implementation sequence:

### Phase 1 --- Project Foundation

-   Create the Tauri v2 + React + TypeScript project.
-   Configure Tailwind CSS.
-   Configure Rust dependencies and Tauri capabilities.
-   Create the initial UI shell and navigation tabs.
-   Confirm the development application starts successfully.

### Phase 2 --- Device Discovery

-   Implement UDP multicast announcements.
-   Implement multicast listening.
-   Add UDP broadcast fallback.
-   Track peers and mark them stale after 15 seconds.
-   Display peers in the frontend.
-   Test discovery with at least two Windows devices on the same Wi-Fi.

### Phase 3 --- File Transfer

-   Implement the HTTP transfer service.
-   Implement Prepare Upload and recipient acceptance.
-   Implement streaming upload/download.
-   Add multi-recipient handling.
-   Emit real-time progress events.
-   Test small files and large movie files.
-   Test decline, cancellation, disconnect, and failure cases.

### Phase 4 --- mpv Integration

-   Add the Windows mpv sidecar.
-   Configure Tauri sidecar packaging and permissions.
-   Launch mpv from the Rust backend.
-   Implement JSON IPC commands and property observation.
-   Implement Play, Pause, Seek, and speed control.
-   Test the packaged application, not only development mode.

### Phase 5 --- Watch Room

-   Implement the WebSocket room host.
-   Implement joining and leaving.
-   Define and validate room message schemas.
-   Broadcast authoritative playback state.
-   Connect room actions to mpv.
-   Display room members and connection status.

### Phase 6 --- Drift Correction

-   Periodically compare client and host positions.
-   Implement the `0.3s` and `1.5s` drift thresholds.
-   Apply temporary speed correction within `0.95x`--`1.05x`.
-   Seek on large drift.
-   Test with multiple devices and variable Wi-Fi conditions.

### Phase 7 --- Installer and Release Validation

-   Configure NSIS packaging.
-   Build using `npm run tauri build`.
-   Verify mpv is included.
-   Install on a clean Windows 10/11 machine.
-   Test without internet access.
-   Test discovery, transfers, and watch rooms across separate devices.
-   Document known limitations and release instructions.

### 6.1 AI-Assisted Coding Rules

When an AI assistant is asked to implement SyncDrop:

1.  Read and follow this specification.
2.  Work on the requested module or phase rather than rewriting the
    entire project unnecessarily.
3.  Provide exact file paths for every created or modified file.
4.  Provide complete source code for requested files, not unexplained
    fragments.
5.  Include exact setup commands and where to run them.
6.  Explain required dependencies and configuration changes.
7.  Do not claim code has been tested unless it has actually been run.
8.  Keep frontend/backend types and network message schemas consistent.
9.  Avoid placeholder implementations for features described as
    complete.
10. If a feature cannot be implemented in the current step, state what
    remains.
11. Preserve working functionality when modifying existing modules.
12. Use current Tauri v2 APIs and configuration.
13. Explain Windows Firewall requirements when local network
    communication is blocked.
14. Never introduce a cloud dependency.
15. Do not silently change ports, protocols, or the media engine.

------------------------------------------------------------------------

## 7. Security, Privacy, and Network Constraints

SyncDrop is designed for a trusted local network, but a LAN is not
automatically secure.

Minimum requirements:

-   Validate all network messages.
-   Validate file metadata and file sizes.
-   Sanitize incoming filenames.
-   Prevent path traversal and arbitrary file writes.
-   Require recipient acceptance before transferring file contents.
-   Avoid silently overwriting files.
-   Keep services bound to local network interfaces as appropriate.
-   Do not expose services through a public relay.
-   Handle malformed packets and unexpected WebSocket messages without
    crashing.
-   Limit concurrent transfers and connections to avoid resource
    exhaustion.
-   Do not execute received files.
-   Do not store unnecessary personal information.
-   Do not treat a peer's nickname or device ID as proof of identity.

Windows Firewall may ask the user to allow SyncDrop to communicate on
private networks. Explain why this permission is needed. Do not advise
users to disable Windows Firewall globally.

------------------------------------------------------------------------

## 8. Error Handling and Edge Cases

The application must handle at least the following:

### Discovery

-   Multicast blocked by router or hotspot.
-   Broadcast unavailable on a network interface.
-   Wi-Fi disconnected or changed.
-   Duplicate device announcements.
-   Stale peers.
-   Invalid or malformed discovery packets.
-   Windows Firewall blocking inbound traffic.

### File Transfer

-   Recipient declines.
-   Recipient goes offline.
-   Sender goes offline.
-   Network connection is interrupted.
-   Disk runs out of space.
-   Destination is not writable.
-   File already exists.
-   File size differs from expected metadata.
-   Multiple recipients respond at different times.
-   One recipient fails while others complete.

### Watch Party

-   Host is unreachable.
-   Host closes the room.
-   Participant disconnects.
-   mpv executable is missing or cannot start.
-   mpv exits unexpectedly.
-   mpv IPC connection fails.
-   Participant has not selected a movie.
-   Movie file metadata does not match.
-   Old or out-of-order synchronization messages arrive.
-   Temporary network latency causes playback drift.

Errors must be visible and understandable to ordinary users. Log
diagnostic details without exposing unnecessary private data.

------------------------------------------------------------------------

## 9. Testing and Acceptance Criteria

A feature is not complete merely because its UI exists. It must work
end-to-end.

### 9.1 Discovery Acceptance

-   Two or more SyncDrop devices on the same Wi-Fi discover each other.
-   Nicknames and device details appear.
-   Peers are updated when announcements arrive.
-   A peer becomes stale/offline after more than 15 seconds without an
    announcement.
-   Broadcast fallback works where multicast is unavailable, subject to
    LAN restrictions.

### 9.2 File Sharing Acceptance

-   A user can select a file and one or more recipients.
-   Every recipient gets an Accept/Decline prompt.
-   A recipient who declines receives no file contents.
-   Accepted recipients receive the complete file.
-   Large files transfer without loading the entire file into memory.
-   Progress, speed, bytes sent, and status update during transfer.
-   One recipient's decline or failure does not prevent another
    recipient's transfer.
-   Received files are saved safely and do not overwrite existing files
    without user consent.

### 9.3 Watch Party Acceptance

-   One device can host a room.
-   Other devices can join by discovered host or host IP.
-   Participants can load their local movie.
-   Play, Pause, Seek, and playback speed changes synchronize.
-   Minor drift is corrected by temporary speed adjustment.
-   Drift greater than 1.5 seconds is corrected by seeking.
-   Room members and connection state are visible.
-   Host departure is handled cleanly.

### 9.4 Packaging Acceptance

-   `npm run tauri build` produces the configured Windows NSIS
    installer.
-   The installer includes the mpv sidecar.
-   The installed app launches on a clean supported Windows system.
-   Normal operation does not require internet access.
-   Users do not need to separately install mpv, Node.js, or Rust.

### 9.5 Group Test

Perform a practical test with approximately 10 devices on the same Wi-Fi
network. Record:

-   Discovery success rate.
-   Transfer throughput and stability.
-   Behavior when several recipients accept simultaneously.
-   Watch-room connection stability.
-   Playback drift under normal Wi-Fi conditions.
-   CPU and memory usage.
-   Firewall and network-isolation limitations.

Do not promise a particular transfer speed or perfect frame-level
synchronization without measurement.

------------------------------------------------------------------------

## 10. Important Design Decisions and Clarifications

These points must be considered during implementation:

1.  **Port sharing:** Discovery uses UDP port `53317`; HTTP transfer is
    TCP and may use the same numeric port because it is a different
    transport protocol, but implementation and firewall behavior must be
    tested. A dedicated transfer port is acceptable if documented.
2.  **LocalSend compatibility:** Using the multicast address and port
    does not by itself guarantee full LocalSend protocol compatibility.
    If actual interoperability with LocalSend is required, implement and
    verify its protocol details. Otherwise, describe the discovery
    mechanism as LocalSend-style rather than claiming protocol
    compatibility.
3.  **Movie identity:** Filename and file size are only a preliminary
    match check. Use SHA-256 when stronger identity verification is
    needed.
4.  **Clock differences:** Do not assume device wall clocks are
    perfectly synchronized. Design drift correction around elapsed time
    and message timing, and document any clock assumptions.
5.  **Host authority:** The host is authoritative for room playback
    state. Clients should not independently broadcast state changes as
    if they were the host.
6.  **Temporary speed correction:** Keep intended playback speed
    separate from the temporary correction multiplier.
7.  **Client isolation:** Some Wi-Fi networks prevent devices from
    communicating with each other. This is a network restriction, not
    necessarily an application defect.
8.  **Windows Firewall:** Inbound UDP, HTTP, and WebSocket traffic may
    require a private-network firewall permission or rule.
9.  **Cross-platform scope:** Tauri is cross-platform, but the initial
    release and bundled mpv binary specified here are Windows x86_64. Do
    not claim macOS or Linux support until their binaries, packaging,
    and tests are implemented.
10. **Transfer protocol:** Use streaming and explicit per-recipient
    state. Do not implement large-file transfer by base64-encoding the
    entire file into JSON.

------------------------------------------------------------------------

## 11. Definition of Done

SyncDrop's initial release is considered complete when:

-   The Windows x86_64 NSIS installer builds successfully.
-   The installed application launches without development tools.
-   The bundled mpv player launches and is controllable through IPC.
-   Devices discover each other on a compatible shared LAN.
-   Users can send files to one or multiple recipients with explicit
    acceptance.
-   Large files transfer through streaming with live progress.
-   Users can host and join a local watch room.
-   Playback actions synchronize across room participants.
-   Drift correction follows the defined thresholds and speed limits.
-   The application works without internet access.
-   Errors and network restrictions are communicated clearly.
-   The project includes setup, build, test, and troubleshooting
    instructions.

------------------------------------------------------------------------

## 12. Source of Truth

This document is the authoritative specification for the SyncDrop
project.

When future requirements are added:

-   Update this file rather than relying only on chat history.
-   Mark new requirements clearly.
-   Keep the current implementation status separate from planned work.
-   Record any approved architecture or protocol changes here.
-   Do not remove existing requirements without an explicit decision.

**Project identity:** SyncDrop\
**Primary purpose:** LAN file sharing + synchronized local movie
playback\
**Initial platform:** Windows 10/11 x86_64\
**Core stack:** Tauri v2 + Rust + React + TypeScript + Tailwind CSS +
bundled mpv\
**Network model:** Direct LAN communication, no cloud dependency


# 13. Advanced Watch Party Requirements — Movie Verification, Late Joining & Playback Continuity

This section defines mandatory Syncplay-style behavior for synchronized movie playback.

These requirements are essential and must be implemented. A participant joining an ongoing room must never automatically restart the movie from the beginning.

## 13.1 Local Movie Requirement

Every participant must have a local copy of the movie they want to watch.

SyncDrop does not need to stream movie playback from the host.

Each laptop plays its own local movie using the bundled mpv media engine.

Example:

- Host has Avengers: Endgame.mp4.
- Friend 1 has Avengers: Endgame.mp4.
- Friend 2 has Avengers: Endgame.mp4.
- Friend 3 has a different movie version.

SyncDrop must verify movie compatibility before allowing normal synchronized playback.

## 13.2 Movie Identity and Verification

When the host creates a Watch Party, the application must publish movie information to the room.

Required FILE_INFO fields:

- filename
- fileSize
- duration
- containerFormat
- videoCodec (when available)
- audioCodec (when available)
- resolution (when available)
- sha256
- movieId

SHA-256 must be calculated asynchronously so that large movie files do not freeze the application.

The hash should be used to establish byte-for-byte file identity.

Verification behavior:

1. Exact SHA-256 match: Mark the movie as verified.
2. Hash mismatch: Warn that the selected movie is not identical.
3. Duration mismatch: Warn that playback timelines may differ.
4. Missing movie: Ask the participant to select a local movie.
5. Unreadable file: Display an error.

For different encodings or releases of the same movie, allow an explicitly approved compatibility mode when the host permits it. Warn that synchronization may be less accurate.

Do not assume matching filenames prove that files are identical.

## 13.3 Participant Movie Loading Workflow

When someone joins a Watch Party:

1. Connect to the host's WebSocket server.
2. Receive room information.
3. Receive the host's FILE_INFO message.
4. Check whether a compatible local movie is already selected.
5. If not, display a Select Movie button.
6. Let the participant browse their local storage.
7. Verify the selected file.
8. Launch the bundled mpv player.
9. Wait until mpv confirms that the movie is loaded.
10. Request the latest authoritative playback state from the host.
11. Seek to the host's current expected position.
12. Apply the correct pause/play state and playback speed.
13. Mark the participant as synchronized.

Do not consider a participant fully synchronized merely because they connected to the WebSocket server.

## 13.4 Late Join Support — Mandatory

SyncDrop must support participants joining an ongoing Watch Party at any time.

Example scenario:

- 10 friends are connected to the same Wi-Fi.
- Five friends start watching Avengers: Endgame.
- The movie reaches 30 minutes.
- Two additional friends join the existing room.
- Both select their local copies of the movie.
- Both movies are verified.
- Both players automatically catch up to the host's current playback position.

The two late participants must start at approximately 30 minutes, NOT 00:00.

If the host is already at 35 minutes when a participant finishes loading the movie, that participant must catch up to approximately 35 minutes.

Never use the original room start position as the late participant's synchronization position.

## 13.5 Authoritative Playback State Snapshot

Implement a dedicated request/response mechanism for late joining.

When a participant finishes loading a movie, it must request the latest room state.

Suggested message types:

- SYNC_REQUEST
- SYNC_SNAPSHOT
- SYNC_READY
- SYNC_CONFIRMED

SYNC_SNAPSHOT must include:

- roomId
- movieId
- position
- paused
- speed
- timestamp
- sequenceNumber
- hostId

The host must calculate the expected playback position at the time the participant is ready to synchronize.

For a playing movie:

Expected position = Snapshot position + elapsed time since snapshot × playback speed.

For a paused movie:

Expected position = Snapshot position.

Account for message delay when calculating the expected playback position.

The participant must:

1. Receive the snapshot.
2. Validate its room and movie identity.
3. Reject stale snapshots.
4. Calculate the expected current position.
5. Seek to the expected position.
6. Apply the host's pause/play state.
7. Apply the intended playback speed.
8. Send SYNC_READY or SYNC_CONFIRMED.
9. Begin normal drift correction.

The host must maintain participant synchronization status.

## 13.6 Play Button Behavior

The Play button must never restart a movie simply because a participant presses it.

Required behavior:

- If the room is already playing, pressing Play must not reset playback.
- If the room is paused, Play requests that the room resume from the current position.
- If a participant has just joined, Play must not override the initial synchronization process.
- If the participant's player is not loaded, Play must not start playback from zero.
- The host validates playback requests and broadcasts the resulting authoritative state.

Use idempotent handling for repeated Play commands.

## 13.7 Playback Control Permissions

Implement a room playback-control policy.

Support:

1. Host Only: Only the host can control playback.
2. Everyone: Every connected participant can request playback controls.

For the initial release, default to Everyone.

Even in Everyone mode, the host remains authoritative.

A participant sends a PLAY, PAUSE or SEEK request. The host validates it, updates the room state and broadcasts the resulting state to every participant.

## 13.8 Late Join State Machine

Implement clear participant states:

1. DISCONNECTED
2. CONNECTING
3. CONNECTED
4. WAITING_FOR_MOVIE
5. VERIFYING_MOVIE
6. LAUNCHING_PLAYER
7. LOADING_MOVIE
8. WAITING_FOR_SYNC
9. SYNCHRONIZING
10. WATCHING
11. OUT_OF_SYNC
12. ERROR

Display understandable status messages in the Watch Party interface.

For example:

- Connecting to room...
- Connected to host.
- Please select Avengers: Endgame.
- Verifying movie...
- Launching media player...
- Catching up with host...
- Synchronized. Enjoy watching!

## 13.9 Missing Movie Workflow

If a participant does not have the movie:

1. Display a missing-movie warning.
2. Offer an option to receive the movie through SyncDrop Wi-Fi Send.
3. Require the sender to initiate the transfer.
4. Require the receiver to accept the transfer.
5. Save the movie locally.
6. Verify the downloaded movie.
7. Load it into mpv.
8. Request a fresh room synchronization snapshot.
9. Join at the current host position.

Never automatically transfer a large movie without explicit consent.

## 13.10 Participant Status Display

The host must be able to see every participant's status.

Examples:

- Connected
- Waiting for movie
- Verifying movie
- Loading movie
- Synchronizing
- Watching
- Out of sync
- Disconnected

Display each participant's current playback position and drift when available.

## 13.11 Host Disconnect and Room Continuity

If the host disconnects:

- Notify all participants.
- Stop accepting the host's old state as current.
- Display a room-disconnected state.
- Do not silently reset playback.
- Allow the application to implement a deliberate host-transfer or room-recreation mechanism in a future release.

For the initial release, ending the room cleanly when the host disconnects is acceptable.

## 13.12 Acceptance Tests for Late Joining

The following tests are mandatory:

TEST 1 — Normal joining:
- Host starts a movie.
- Another participant joins.
- Both synchronize.

TEST 2 — Late joining at 30 minutes:
- Host and four friends watch for 30 minutes.
- Another participant joins.
- The participant loads the correct movie.
- Playback starts around 30 minutes.

TEST 3 — Late joining while paused:
- Host pauses at 30 minutes.
- A participant joins.
- The participant loads the movie.
- Their player seeks to 30 minutes and remains paused.
- When the host resumes, the participant resumes too.

TEST 4 — Host continues playing during loading:
- Participant begins loading the movie when host is at 30 minutes.
- Movie loading takes 20 seconds.
- Host continues playing.
- Participant catches up to the host's expected position after those 20 seconds.

TEST 5 — Different movie:
- Participant selects an incompatible movie.
- SyncDrop displays a verification warning.
- The participant cannot silently enter normal synchronized playback.

TEST 6 — Repeated Play:
- Room is already playing.
- Participant presses Play.
- The movie must not restart.

TEST 7 — Seek:
- Host seeks to 45 minutes.
- Every synchronized participant follows.

TEST 8 — Missing movie:
- Participant does not have the movie.
- Participant receives it through Wi-Fi Send with consent.
- Participant verifies and loads it.
- Participant joins the current playback position.

These tests must pass before declaring the Watch Party implementation complete.