# Where this got to

Working notes for flashDK itself. Delete or rewrite the sections below once they no
longer reflect a moving target: the per-vendor picture, once all four vendors have
settled adapters, and the open questions, once they're answered.

**Last updated:** 2026-08-26.

## What is true right now, per vendor

| Vendor | HID | Power | Virtual media | Notes |
|---|---|---|---|---|
| PiKVM | Live, verified | Live, verified reads; actions implemented but never exercised (this unit has no ATX controller attached) | Live, verified | TLS with trust-on-first-use pinning; see [security.md](security.md) |
| NanoKVM | Live, verified | Live, verified | Live, verified | Cleartext HTTP; see [security.md](security.md) for what that means in practice |
| JetKVM | Live, verified over WebRTC | Not implemented | Not implemented | The `rpc` JSON-RPC channel works (verified with `getLocalVersion`), but the method names and payload shapes for power and virtual media haven't been captured off the wire yet |
| GL.iNet | Live, verified (key/mouse/wheel/paste all confirmed against the real API) | Live API accepted (`power`/`power_long`/`reset` all return `ok:true`); no host attached to the capture port, so no downstream effect observed yet | Live, verified reads; mount/connect not exercised (no image available on this fresh unit) | Turns out to run the `kvmd` daemon stack itself, confirmed via its own `/api/info`; its own login-and-token auth, not PiKVM's static headers. See [captures/glinet-comet-kvmd-api.md](captures/glinet-comet-kvmd-api.md) |

"Verified" means exercised against a real, owned device and the result checked, not
just compiled. Every adapter also carries unit tests pinning its wire encoders to the
exact bytes captured live, independent of whether the device happens to be reachable
on a given day.

## Beyond KVMs: power infrastructure

`PowerOutlet` and `UpsStatus` (`flashdk-core`) are scaffolded per
[ROADMAP.md](ROADMAP.md); no adapter implements `PowerOutlet` yet. A NUT client
(`flashdk_adapters::nut`) implements `UpsStatus`, built from IETF RFC 9271 and
NUT's official manual pages rather than any device or NUT's own source. Its
protocol encoder and parser are unit-tested against the RFC's own literal example
text, real verification of the wire format, but the TCP client has not been
exercised against a running `upsd` or a real UPS: this project has neither
available yet. Different confidence tiers within the same adapter, worth being
precise about rather than calling the whole thing either "done" or "untested":
`battery.charge`, `ups.status`, and `test.panel.start` are quoted directly in the
RFC text; `ups.load`, `battery.runtime`, and the beeper commands are corroborated
independently but not RFC-primary. See
`crates/flashdk-adapters/src/nut/PROVENANCE.md`.

Ubiquiti UniFi PDU: checked, not built. The official Network Integration API's
published OpenAPI schema (`developer.ui.com/network`, v10.4.57) has no PDU
concept at all: `features` is exactly `switching`/`accessPoint`/`gateway`,
`interfaces` is exactly `ports`/`radios`, and there is no `outlet`, `relay`, or
`pdu` anywhere in its paths or component schemas. An earlier `RELAY` grep hit
was a false positive (DHCP Relay Configuration). The only device-control
actions the schema documents are a generic `RESTART` and a per-port PoE
`POWER_CYCLE`, neither of which is outlet-specific PDU control. Real outlet
control almost certainly exists only via the older, undocumented internal
Controller API, which is community-reverse-engineered and doesn't clear
[CLEANROOM.md](../CLEANROOM.md)'s bar. Status: blocked on either Ubiquiti
documenting it officially or this project acquiring a UniFi PDU to probe
directly. See `docs/ROADMAP.md`'s "Ubiquiti UniFi PDU" section for the full
writeup.

## Beyond KVMs: enterprise BMCs (Redfish)

A first-pass `redfish` adapter (`flashdk_adapters::redfish`) now implements
`Power` (via `#ComputerSystem.Reset`) and `VirtualMedia` (via
`#VirtualMedia.InsertMedia`/`EjectMedia`), built directly from the official
DMTF DSP8010 JSON Schema bundle, downloaded and read as raw JSON rather than a
summarized fetch, the same discipline the NUT adapter's RFC reading used. The
`ResetType` enum, the `Reset`/`InsertMedia`/`EjectMedia` action shapes, and the
`UserName`/`Password` session fields are all confirmed directly against
schema files (`ComputerSystem.v1_28_0.json`, `Resource.json`,
`VirtualMedia.v1_6_5.json`, `Session.v1_7_0.json`, `ServiceRoot.v1_18_0.json`).
The session-login handshake itself (`POST` to `SessionService/Sessions`,
token returned via the `X-Auth-Token` header) is documented in the core
DSP0266 specification rather than a schema file, so it's corroborated across
two DMTF documents rather than quoted from one. See
`crates/flashdk-adapters/src/redfish/PROVENANCE.md`.

Compiles and passes unit tests against the schema's literal values (7 tests in
`redfish::protocol`), but the end-to-end HTTP flow (login, root/system/manager
discovery, action dispatch) has not been exercised against a real BMC: this
project has neither an iDRAC nor an iLO unit on hand. Serial console over
Redfish isn't implemented yet, and each vendor's own graphical console (a
separate, proprietary layer per `docs/ROADMAP.md`) is out of scope entirely.

## What is not built yet

- **JetKVM power and virtual media.** The transport is live and the `rpc` channel
  works; only the specific method names and argument shapes remain uncaptured.
- **A GL.iNet adapter.** Blocked on hardware, not on anything technical.
- **A UniFFI layer.** Nothing generates Swift or Kotlin bindings yet; the crates are
  Rust-only consumers today (see the example binaries under
  `crates/flashdk-adapters/examples/`).
- **TOFU pinning beyond PiKVM and Redfish.** The mechanism in `tls_pin.rs` is
  reused as-is by `redfish::RedfishBmc`; NanoKVM has no TLS to pin, and JetKVM's
  HTTP signaling is unauthenticated by transport (its media is secured by
  WebRTC's own DTLS instead).

## What the previous notes got wrong

Recorded rather than quietly corrected, because the same kind of mistake is likely
to recur on the next vendor.

- **JetKVM's HID was assumed to be JSON-RPC.** Early probing of the vendor's
  frontend bundle surfaced method names like `keyboardReport` and
  `absMouseReport`, which read as JSON-RPC method calls. A live DataChannel capture
  showed the opposite: those names describe the *capability*, not the wire format.
  Keyboard and mouse actually ride separate **binary** `hidrpc*` channels with a
  compact custom encoding, and JSON-RPC is used only for the unrelated `rpc`
  control channel. The lesson that stuck: a method name glimpsed in a bundle is a
  hypothesis, not a capture, and only a real DataChannel hook settled it.
- **NanoKVM's WebRTC support was stated as a flat "no."** The initial capability
  read `video_webrtc: false` based on the REST endpoint list alone. NanoKVM's
  `server.yaml` was later found to carry `stun:`/`turn:` configuration, meaning
  WebRTC is plausibly supported and simply wasn't exercised in the REST probe. The
  capability now reads as unverified rather than false; neither claim has been
  confirmed by an actual WebRTC handshake against the device.

## Open questions

- **Does JetKVM's `rpc` channel expose power and virtual media the same way kvmd
  does, or with a different shape?** Unknown until captured. The channel and the
  request/response plumbing are already built; only the method catalogue is
  missing.
- **Is NanoKVM's WebRTC support real or vestigial config?** The `stun`/`turn`
  fields could be inherited defaults never wired to anything. Settling this needs a
  capture with the video panel actually active, not just a config read.
- **Should PiKVM's cached identity refresh (`refresh_identity`) run automatically
  on connect, or stay opt-in?** Currently opt-in, because `Device::info()` is
  documented as a cheap, no-network call and an automatic fetch would either break
  that contract or hide a network round-trip inside a constructor. Revisit if a
  consuming app finds the manual call surprising in practice.
