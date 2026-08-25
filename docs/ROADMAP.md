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

## Planned: power infrastructure (PDU and UPS)

A third device class beyond KVMs and BMCs. Neither a PDU nor a UPS has HID or
video, but both fit the same capability-based model through outlet control and
power monitoring. Adding this class nudges flashDK's identity from "KVM manager"
toward "controllable infrastructure" more broadly.

### Ubiquiti UniFi PDU

Controlled through the UniFi Network controller API, either a self-hosted
controller or a UniFi OS console such as a Dream Machine or Cloud Key. Modern
UniFi OS (4.x) exposes a local API-key REST API, with outlet on/off/cycle done
through per-outlet device management overrides; the older path is a controller
session login followed by a `rest/device` PATCH. This fits a `PowerOutlet`
capability: enumerate outlets, switch them, and read per-outlet metering on
models that support it. UniFi's API is documented well enough to implement to
the documented surface directly, and it's TLS-pinnable.

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

### What this implies for core

Two new capability traits: `PowerOutlet` (enumerate, switch, and meter multiple
outlets) and `UpsStatus` (read-only telemetry plus the small set of commands a
UPS actually supports). The existing `Power` trait continues to cover
whole-machine ATX control; outlets and UPS telemetry are additive capabilities
behind their own flags, keeping the single capability-negotiated model
consistent across KVMs, BMCs, PDUs, and UPS devices.
