# Architecture

How flashDK is put together, and why each piece is where it is. For why specific
technology choices were made rather than alternatives, see
[decisions.md](decisions.md); this document covers structure, not selection.

## Two crates, one boundary

```
flashdk-core        vendor-neutral traits and types. Zero dependencies.
flashdk-adapters     one module per vendor, implementing core's traits.
```

The boundary is deliberate and enforced by what each crate is allowed to know.
`flashdk-core` has never seen a vendor name; it defines what a KVM *can do*
(`Hid`, `Power`, `VirtualMedia`, `Device`) without knowing how any particular
device does it. `flashdk-adapters` depends on `flashdk-core` and nothing else
knows about a specific vendor's wire format.

This means a consuming app writes to `flashdk-core`'s traits and the `Kvm`
dispatcher, never to a specific adapter's internals, so adding a fifth vendor
never touches app code.

## The capability model

Every device exposes a `Capabilities` struct at connect time: which of keyboard,
absolute mouse, power reset, and so on it actually supports. The rule this
encodes, learned directly from probing real devices, is to assume nothing and
check everything. NanoKVM's GPIO-based power control and PiKVM's ATX line both
answer to the same `Power` trait, but only one of them can do a true reset, and
an app that assumes both can is going to be wrong on real hardware. See
[STATE.md](STATE.md) for the current per-vendor capability picture.

## The transport split

The single most consequential structural fact in this codebase: the target
devices don't share one transport shape.

- **PiKVM and NanoKVM** are request/response. A keystroke is one self-contained
  HTTP call (PiKVM) or one WebSocket frame (NanoKVM); nothing needs to be
  negotiated first.
- **JetKVM** inverts this. Control rides a WebRTC `RTCDataChannel`, so a peer
  connection has to be fully negotiated, over signaling, ICE, and DTLS, before a
  single keystroke can be sent. The peer connection isn't carrying video
  alongside control; for JetKVM it *is* the control channel.

`flashdk_core::TransportKind` names this split explicitly (`RequestResponse` vs
`PeerRpc`) rather than papering over it, because an app needs to know which one
it's dealing with to decide how much setup work "connect" actually implies.

## Per-vendor adapter shape

Each adapter under `crates/flashdk-adapters/src/<vendor>/` follows the same
internal pattern, even though the wire formats differ completely:

- `mod.rs`: implements `flashdk-core`'s traits, holding whatever connection
  state the vendor's protocol needs (an HTTP client and credentials for PiKVM; a
  token and an open WebSocket for NanoKVM; a `JetTransport` handle for JetKVM).
- A `wire` module (or equivalent): pure functions that encode and decode the
  vendor's actual byte format, kept separate from the async networking code so
  they can be unit-tested against literal captured bytes without needing a live
  device or a network stack.
- `PROVENANCE.md`: the clean-room attestation required by
  [CLEANROOM.md](../CLEANROOM.md).

JetKVM additionally carries `transport.rs`, the sans-IO WebRTC driver, because
its transport complexity doesn't belong inside the adapter's own logic.

## Dispatch without `dyn`

The capability traits use `async fn`, which makes them not object-safe: you
cannot write `Box<dyn Device>` and call it a day. Runtime polymorphism across
vendors instead goes through a plain `Kvm` enum in `flashdk-adapters::lib`, with
one variant per vendor and a `match` in every method. This is not a workaround;
it is faster than dynamic dispatch, and the compiler refuses to compile a new
vendor addition until every method's `match` has been updated to handle it,
which a trait object would not enforce.

## Certificate pinning is shared, not per-adapter

`tls_pin.rs` lives in `flashdk-adapters` directly, not inside `pikvm/`, because
PiKVM is not expected to be the only TLS-pinnable device forever. The `PinStore`
trait and `TofuVerifier` are vendor-agnostic; any future adapter that needs
trust-on-first-use pinning against a self-signed certificate reuses them
unchanged. See [security.md](security.md) for the reasoning behind pinning
itself.
