SyncDrop - End User License Agreement & Offline Privacy Policy
Version 0.1.0

================================================================================
1. OFFLINE & PRIVACY FIRST COMMITMENT
================================================================================
SyncDrop is designed from the ground up as a zero-cloud, 100% offline, privacy-first
application.

- NO CLOUD TELEMETRY: SyncDrop does not collect, record, track, or transmit your
  personal information, files, metadata, device identifiers, or usage habits to any
  external server, third-party entity, or cloud infrastructure.
- ZERO INTERNET REQUIRED: SyncDrop operates strictly within your local area network (LAN)
  or Wi-Fi subnet. An internet connection is neither required nor utilized.
- NO AUTOMATIC FILE TRANSFERS: Files and media are never transferred without explicit
  sender initiation and explicit recipient acceptance.
- LOCAL CRYPTOGRAPHIC HASHING: File integrity and movie verification checks (SHA-256)
  are performed entirely on your local machine's processor.

================================================================================
2. LOCAL NETWORK USAGE & FIREWALL REQUIREMENTS
================================================================================
SyncDrop communicates with other SyncDrop devices on your local Wi-Fi subnet using
the following documented local network ports:

- UDP Port 53317: Used for local device discovery (multicast address 224.0.0.167
  and subnet broadcast fallback).
- TCP Port 53317: Used for direct peer-to-peer streaming file transfers between
  devices on your local Wi-Fi.
- TCP/WebSocket Port 53318: Used for real-time Watch Party playback synchronization.

When running SyncDrop for the first time, Windows Defender Firewall will ask for
permission to communicate on "Private networks". Granting this permission is necessary
for SyncDrop to discover and synchronize with your peers on your local Wi-Fi.

================================================================================
3. LICENSE GRANT & PERMITTED USE
================================================================================
Permission is hereby granted to use, copy, and run SyncDrop for personal, non-commercial,
or commercial purposes on compatible Windows operating systems, subject to the
condition that you do not hold the developers liable for any data loss or hardware
malfunction resulting from local network operations.

================================================================================
4. THIRD-PARTY COMPONENTS
================================================================================
SyncDrop incorporates the mpv media player (mpv.io) as an embedded engine sidecar,
distributed under the GNU General Public License (GPLv2/GPLv3).

By clicking "I Agree" or continuing with the installation, you acknowledge and agree
to these terms.
