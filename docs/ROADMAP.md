# flashDK Roadmap

## Current adapters

PiKVM has HID, power (ATX), and virtual media over its REST API, all live and
verified. NanoKVM has the same three, live and verified, over a binary WebSocket
protocol with a Rust reimplementation of its AES-based login. JetKVM has live,
verified HID over WebRTC DataChannels, built on the sans-IO `str0m` stack; its
power and virtual media ride the same connection's JSON-RPC `rpc` channel but
aren't implemented yet, because the specific method names and payload shapes
haven't been captured off the wire. GL.iNet's Comet (GL-RM1/PE) has no adapter,
pending hardware.

## Planned: enterprise out-of-band management (BMCs)

A distinct device class from HDMI-capture KVMs, but one that fits the same
capability-based adapter model, and much of it is standards-based enough to be
clean-room by construction: implementing a published specification rather than
reverse-engineering a vendor's firmware.

### Dell iDRAC and HPE iLO, via Redfish

Redfish (a DMTF open standard, REST/JSON over HTTPS) covers power actions, system
state, virtual media, and serial console in a way that's standardized across both
vendors, so a single `redfish` adapter could serve both iDRAC and iLO for power,
media, and serial. The graphical console is the part that stays proprietary:
iDRAC and iLO's virtual console is each vendor's own HTML5 viewer, not
standardized, so video and HID there need per-vendor work in the same way the
hobbyist KVMs do. Redfish sessions authenticate over TLS, which is pinnable, a
real improvement over the cleartext hobbyist devices this project started with.

### Intel vPro AMT, via AMT KVM redirection

AMT exposes a VNC/RFB-compatible remote desktop on ports 16992 to 16995, RFB
being an open protocol, with AMT's own authentication in front of it. That maps
cleanly onto HID plus a framebuffer video source. Power control is available
through AMT's own remote power control (WS-MAN, or Intel's published API).
AMT's provisioning and security posture varies a great deal across deployments,
which is worth surfacing honestly to a user rather than presenting a single
"secure" state, consistent with how this SDK already treats transport security
per vendor; see [security.md](security.md).

**Sourced, not yet built.** Checked directly against Intel's own published
documentation (the AMT SDK Implementation and Reference Guide and the AMT
Developer's Guide), not any third-party write-up:

- Power control is `CIM_PowerManagementService.RequestPowerStateChange`, a
  WS-Management method taking a `PowerState` value and a reference to the
  managed `CIM_ComputerSystem`. The always-supported values are documented
  verbatim: 2 (Power Up), 5 (Power Cycle), 8 (Power Down), 10 (Reset), with
  4 (Sleep-Deep), 7 (Hibernate), 12 (Power Off-Soft Graceful), and 14 (Master
  Bus Reset Graceful) available "pending OS capabilities." `TimeoutPeriod`
  only accepts `0`; `Time` isn't supported at all, both explicitly called out
  in the method's own documented caveats.
- KVM redirection needs to be switched on first via a separate WS-Management
  setting (`IPS_KVMRedirectionSettingData`) before a session can connect at
  all; this isn't optional plumbing, Intel's own docs describe it as a
  precondition.
- Once enabled, video/HID rides RFB (an IETF-documented protocol family) over
  Intel's own redirection ports: 16994 plaintext / 16995 TLS speak a
  vendor-extended "RFB 4.0," while the standard RFB 3.8 is available on the
  IANA VNC port 5900 if separately enabled with its own password. Ports
  16992/16993 (TLS) carry the authentication handshake for the redirection
  ports.

Every fact above traces to Intel's own SDK documentation, most of it read via
the Internet Archive's Wayback Machine after `software.intel.com` (the
original host) started returning HTTP 403 to automated fetches; the archived
pages are still Intel's own published text, not a third party's summary of
it, so this stays within [CLEANROOM.md](../CLEANROOM.md)'s bar. What isn't
sourced yet: the exact SOAP envelope shape WS-Management itself expects
(WS-Management is its own DMTF standard, DSP0226/DSP0227, not yet read
directly) and the redirection-port authentication handshake's byte-level
framing, both needed before code can be written, not just documentation.

**Protocol-version wrinkle, worth flagging before probing any real host:**
WS-Management wasn't part of AMT from the start. The project's one real
vPro-capable unit on hand reports AMT 1.2, which predates WS-Management
entirely; sourcing above (`CIM_PowerManagementService`, WS-Management framing)
applies to AMT 3.0 and later, which is what the overwhelming majority of
currently-deployed vPro hardware actually runs, and is where this adapter's
near-term effort stays targeted. Older AMT generations (1.x/2.x) used a
different, Intel-proprietary SOAP schema on the same ports rather than
WS-Management, so a capture against the 1.2 unit would tell us about that
older dialect, not the one most users have. Two open-source tools exist that
speak to older/varied AMT generations (`amttool`, part of the `openhpi`/ELRepo
lineage, and [sdague/amt](https://github.com/sdague/amt) on GitHub); both are
noted here only as evidence that the protocol genuinely forked across AMT
versions, not as a source: both are GPL-licensed and neither has been opened
or read, per [CLEANROOM.md](../CLEANROOM.md). If the 1.2 unit ever gets
probed, it needs its own from-scratch capture and its own PROVENANCE.md
entry, kept separate from the WS-Management-based work above rather than
blended into it.

This project also has no AMT 3.0+ hardware on hand to verify the sourced work
above against. Status: **sourced, not started**, the tier before Redfish was
in prior to this session's work.

### Why these are a good fit

Power, virtual media, and serial are more standardized here, through Redfish,
than on any of the hobbyist KVMs, so they slot directly into the existing
`Power` and `VirtualMedia` traits without a new abstraction. Redfish, RFB/VNC,
and WS-MAN are all open, published standards, so these adapters can be written
to specification rather than reverse-engineered, which is the strongest
provenance this project can have. TLS-pinnable authentication across the board
strengthens the security story further.

### Sequencing

GL.iNet first, since it closes out the hobbyist-KVM set already underway. Then a
`redfish` adapter covering iDRAC and iLO power, media, and serial, which is high
value and standards-based enough to move quickly. AMT after that, with the
proprietary graphical consoles (iDRAC, iLO, and AMT's own) as a later push once
the standards-based groundwork is in place.

A first pass at the `redfish` adapter (`flashdk_adapters::redfish`) now exists,
covering `Power` (via `#ComputerSystem.Reset`) and `VirtualMedia` (via
`#VirtualMedia.InsertMedia`/`EjectMedia`), built directly from the official
DSP8010 JSON Schema bundle and corroborated against DSP0266 for the session-login
handshake; see `crates/flashdk-adapters/src/redfish/PROVENANCE.md`. Serial console
over Redfish is not implemented yet. This project has no iDRAC or iLO to test
against, so the whole adapter is compiled-and-unit-tested, not verified live; see
[STATE.md](STATE.md).

## Planned: power infrastructure (PDU and UPS)

A third device class beyond KVMs and BMCs. Neither a PDU nor a UPS has HID or
video, but both fit the same capability-based model through outlet control and
power monitoring. Adding this class nudges flashDK's identity from "KVM manager"
toward "controllable infrastructure" more broadly.

### Ubiquiti UniFi PDU

**Checked and corrected:** the earlier version of this section assumed outlet
control was reachable through official, documented per-outlet overrides. It
isn't, at least not yet. Ubiquiti's official Network Integration API (the
published OpenAPI schema at `developer.ui.com/network`, checked at v10.4.57)
has no PDU concept anywhere in it: a device's `features` enum is exactly
`switching`, `accessPoint`, `gateway`, and its `interfaces` enum is exactly
`ports`, `radios`. No `outlet`, no `relay`, no `pdu`, in either the paths or
the component schemas. The two device-control primitives that do exist are a
generic per-device `RESTART` action and a per-port `POWER_CYCLE` action (aimed
at cycling PoE power on a switch port), neither of which is outlet-specific
PDU control.

What outlet-level control the PDU almost certainly has comes from the older,
undocumented internal Controller API (a `rest/device` PATCH with an
`outlet_overrides` field, per community reverse-engineering, not Ubiquiti's
own documentation). Building against that would mean sourcing the adapter
from community write-ups whose own provenance traces back to observing or
decompiling the controller's behavior, which doesn't clear the bar
[CLEANROOM.md](../CLEANROOM.md) sets: wire observation against a device this
project owns, or official documentation, not someone else's write-up of
either. This project doesn't own a UniFi PDU to probe directly yet, so
there's nothing to build against cleanly in either direction right now.

**Status: blocked**, not on hardware, but on there being no clean-room-safe
source for outlet control at all until either Ubiquiti documents it
officially or a unit is available to probe the wire directly (which would
then be a legitimate, capture-based path, the same one every KVM adapter
took). The generic `RESTART` and `POWER_CYCLE` actions are officially
documented and could support a narrower, honest adapter later if a PDU or a
UniFi switch is on hand to confirm they apply the way the schema implies.

### APC Back-UPS 1500

This is a consumer UPS, USB-only with no network port of its own, so it isn't
directly network-controllable. The path is a host running NUT (Network UPS
Tools, `upsd` on TCP 3493) or apcupsd (NIS on port 3551) with the UPS attached
over USB. Its capabilities are monitoring-first: charge percentage, load,
line-power versus on-battery status, and a runtime estimate. Control is limited
by the hardware itself; NUT's `instcmd` can do things like mute the beeper or
run a self-test, but the Back-UPS has no switched outlets, so there's no
per-outlet control to expose. NUT's network protocol is an open, documented
standard, so an adapter here is implemented to spec as well. For genuinely
switchable, networked UPS control, an APC Smart-UPS with a Network Management
Card (SNMP or Redfish) is the upgrade path; the Back-UPS itself stays
monitor-plus-limited-commands.

A NUT client (`flashdk_adapters::nut`) implementing `UpsStatus` already exists,
built directly from IETF RFC 9271 (the published NUT network protocol) and NUT's
official manual pages, not from NUT's own source. It compiles and its protocol
encoder/parser are unit-tested against the RFC's own example text, but the
end-to-end TCP client has not been exercised against a running `upsd` or a real
UPS: this project doesn't have either on hand yet. See
`crates/flashdk-adapters/src/nut/PROVENANCE.md` and
[STATE.md](STATE.md) for what's confirmed versus corroborated.

### What this implies for core

Two new capability traits: `PowerOutlet` (enumerate, switch, and meter multiple
outlets) and `UpsStatus` (read-only telemetry plus the small set of commands a
UPS actually supports). The existing `Power` trait continues to cover
whole-machine ATX control; outlets and UPS telemetry are additive capabilities
behind their own flags, keeping the single capability-negotiated model
consistent across KVMs, BMCs, PDUs, and UPS devices.

## Planned: hypervisor and NAS host control

A fourth device class: not a KVM, BMC, PDU, or UPS, but still "controllable
infrastructure" in the same sense, and a natural fit once flashDK already
models power and virtual media as capability traits rather than KVM-specific
concepts. Both entries below run on hardware this project already has on
hand (the same Dell OptiPlex used as JetKVM's redirection target runs
Proxmox), so once scoped, these are buildable against a real, owned host
rather than only against documentation.

### Proxmox VE

Proxmox VE's own REST API is officially documented (`pve-docs`, published by
Proxmox Server Solutions GmbH) and covers per-VM/container power actions
(start/stop/shutdown/reset), console access proxied through its own
noVNC/SPICE/xterm.js gateway, and host-level status. Proxmox VE itself is
AGPL-3.0-licensed, so the same clean-room discipline applies as everywhere
else in this project: build from the published API documentation, never from
Proxmox's own source, to keep flashDK's Apache-2.0 license intact. The
per-VM model (many power-controllable "machines" behind one host, rather than
one machine per adapter instance) doesn't map onto `Device`/`Kvm` the way a
single-target KVM does, so this likely needs its own thin collection type
rather than forcing a single `Power` impl per Proxmox node. Not sourced yet.

### Unraid

Unraid exposes host control (array start/stop, VM and container power
actions, and increasingly a documented GraphQL API on recent releases)
alongside its more traditional web UI. Unraid itself is closed-source, so
unlike Proxmox this one depends entirely on what Unraid publishes as official
API documentation; if the GraphQL schema (or its predecessor) is documented
well enough to build against without reading Unraid's own code, this stays
clean-room by the same rule as Redfish and NUT. Not sourced yet; needs a
documentation pass before any code, the same way the UniFi PDU section above
needed one before concluding it was blocked rather than built.
