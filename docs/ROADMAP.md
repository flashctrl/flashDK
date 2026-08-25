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

## Planned: power infrastructure (PDU / UPS)

A third device class beyond KVMs and BMCs — no HID/video, but they slot into a
capability-based model (outlet control + power monitoring). This nudges flashDK's
identity toward "controllable infrastructure," not just KVMs.

### Ubiquiti UniFi PDU (e.g. USP-PDU / PDU Pro) — user has one
- Controlled via the **UniFi Network controller API** (self-hosted controller or a
  UniFi OS console — Dream Machine / Cloud Key). Modern UniFi OS (4.x) exposes a local
  **API-key** REST API; outlet on/off/cycle is done through device management
  (per-outlet overrides). Older path: controller session login + `rest/device` PATCH.
- Fits a `PowerOutlet` capability (enumerate outlets, on/off/cycle, read per-outlet
  metering on metered models).
- Clean-room: UniFi's API is documented (official API keys + community references);
  implement to the documented REST surface. TLS-pinnable.

### APC Back-UPS 1500 (BX/BN1500) — user has one
- **Consumer UPS: USB only, no network port.** Not directly network-controllable.
  Reached via a host running **NUT (Network UPS Tools, `upsd` on TCP 3493)** or
  **apcupsd** (NIS on 3551) with the UPS on USB.
- Capabilities are monitoring-first: charge %, load, line/on-battery status, runtime
  estimate. Control is limited by the hardware — NUT `instcmd` can do things like
  beeper mute and self-test; Back-UPS lacks switched outlets, so no per-outlet control.
- Clean-room: NUT's network protocol is an open, documented standard — implement to spec.
- Note: for switchable/networked UPS control, an APC **Smart-UPS + Network Management
  Card** (SNMP/Redfish) is the upgrade path; the Back-UPS is monitor + limited commands.

### Core implication
Add capability traits for `PowerOutlet` (multi-outlet on/off/cycle, metering) and
`UpsStatus` (read-only telemetry + limited commands). The existing `Power` trait covers
whole-machine ATX; outlets and UPS telemetry are new, additive capabilities behind
flags — keeping the single capability-negotiated model across KVMs, BMCs, PDUs, and UPS.
