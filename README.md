# flashDK

A clean-room, **Apache-2.0** SDK for controlling IP-KVM devices across vendors
(Sipeed NanoKVM, PiKVM, JetKVM; GL.iNet planned). One protocol implementation, meant to
be embedded in native apps via UniFFI (Swift/Kotlin) and freely usable by any client —
proprietary or open.

> Built entirely from wire observation and official docs. Never from anyone's source.
> See [CLEANROOM.md](CLEANROOM.md).

## Layout

```
crates/
  flashdk-core/       # vendor-neutral model: traits + types, zero dependencies
    src/
      capability.rs     # Capabilities (what a device supports) + Vendor
      transport.rs      # TransportKind — REST/WS vs WebRTC-DataChannel-RPC (key split)
      hid.rs            # keyboard/mouse contract — the FIRST layer we make real
      power.rs          # power actions/state
      media.rs          # virtual media (mount ISO/image)
      device.rs         # Device umbrella trait + DeviceInfo
      error.rs          # one Error type
  flashdk-adapters/   # one module per vendor, implements the core traits
    src/
      nanokvm/  pikvm/  jetkvm/     (+ PROVENANCE.md each)
      lib.rs            # `Kvm` enum — runtime dispatch across vendors, no `dyn`
docs/captures/          # raw wire-capture evidence (the clean-room audit trail)
```

## Reading order (for learning)

`flashdk-core/src/lib.rs` → `capability.rs` → `transport.rs` → `hid.rs` →
`device.rs`, then `flashdk-adapters/src/lib.rs` and one adapter. The doc-comments
(`//!` and `///`) explain the *why*, not just the *what*.

## Status

**Scaffold.** Every adapter action returns `Error::NotImplemented`; the shape is real,
the behaviour isn't. Next: make HID real against a live NanoKVM/PiKVM.

## Build

```bash
# Install Rust once (if you haven't):
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Then, from this directory:
cargo check     # type-check the whole workspace
cargo doc --open --no-deps   # read the annotated API in your browser
```

## What comes later
- Real HID over each transport (HTTP/WS for Nano/PiKVM, DataChannel RPC for JetKVM)
- Async runtime (`tokio`, MIT) + HTTP client + a BSD-licensed WebRTC stack (`webrtc-rs`)
- A UniFFI layer generating Swift/Kotlin bindings from these same traits
- TOFU certificate pinning (PiKVM), honest per-device security state
