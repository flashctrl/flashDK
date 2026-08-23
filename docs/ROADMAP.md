# flashDK Roadmap

## Current adapters
- **PiKVM** — HID + power (ATX) + virtual media (REST). Live.
- **NanoKVM** — HID (binary WS) + power + virtual media, AES login (RustCrypto). Live.
- **JetKVM** — HID live over WebRTC DataChannels (str0m sans-IO). Power/virtual media
  (JSON-RPC `rpc` channel) pending.
- **GL.iNet Comet (GL-RM1/PE)** — pending hardware (shipping soon).

## Planned: enterprise out-of-band management (BMCs)

A distinct device class from HDMI-capture KVMs, but it fits the same capability-based
adapter model — and much of it is **standards-based**, so it's largely clean-room by
construction (implementing published specs, not reverse-engineering firmware).

### Dell iDRAC / HPE iLO — via Redfish (+ IPMI legacy)
- **Redfish** (DMTF open standard, REST/JSON over HTTPS) covers power actions, system
  state, virtual media (`VirtualMedia`), and serial console — standardized across both
  vendors. A single `redfish` adapter can serve iDRAC and iLO for power/media/serial.
- **Graphical console (KVM)** is the proprietary part: iDRAC/iLO virtual console is an
  HTML5/vendor viewer, not standardized — video/HID here needs per-vendor work and is
  the hard bit (as with the hobbyist KVMs).
- Auth: Redfish sessions (token) over TLS — pinnable, unlike the cleartext hobbyist KVMs.

### Intel vPro AMT — via AMT KVM redirection
- **KVM**: AMT exposes a VNC/RFB-compatible remote-desktop on ports 16992–16995 (RFB is
  an open protocol) with AMT's own auth — maps cleanly to HID + framebuffer video.
- **Power**: AMT remote power control (WS-MAN / published Intel API).
- Note: AMT provisioning/security posture varies; surface it honestly per the app's
  principled-security stance.

### Why these are a good fit
- **Power/virtual media/serial** are *more* standardized here (Redfish) than on the
  hobbyist KVMs — they slot straight into core's `Power`/`VirtualMedia` traits.
- **Clean-room**: Redfish (DMTF), RFB/VNC, and WS-MAN are open published standards, so
  adapters can be written to spec rather than reverse-engineered — the strongest
  possible provenance.
- **TLS-pinnable** auth across the board, strengthening the security story.

### Sequencing
Land GL.iNet first (closes the hobbyist set), then a `redfish` adapter (iDRAC + iLO
power/media/serial — high value, standards-based, quick), then AMT, then the
proprietary graphical consoles as a later push.
