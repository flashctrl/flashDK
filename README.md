# flashDK

[![CI](https://github.com/flashctrl/flashDK/actions/workflows/ci.yml/badge.svg)](https://github.com/flashctrl/flashDK/actions/workflows/ci.yml) [![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

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

## Trademarks & interoperability

flashDK is an **independent, clean-room interoperability library**. It is not affiliated
with, endorsed by, or sponsored by any device vendor.

PiKVM, NanoKVM, Sipeed, JetKVM, and GL.iNet are trademarks of their respective owners.
Those names appear here **only nominatively** — to state which devices flashDK can talk
to. They are never used as flashDK's own branding or package identity, and CI enforces
that (see `.github/workflows/ci.yml`).

Every adapter is implemented solely from observed network behaviour and official public
documentation, never from a vendor's source code — see [CLEANROOM.md](CLEANROOM.md) and
each adapter's `PROVENANCE.md`. This is what keeps the SDK licensable under Apache-2.0
independent of the vendors' (copyleft) firmware licenses.

## Compliance (enforced in CI)

Every push and PR must pass:

| Gate | Guards against |
|------|----------------|
| `cargo-deny` licenses | a copyleft (GPL/LGPL) dependency forcing the SDK open |
| `cargo-deny` advisories | shipping a known-vulnerable or yanked dependency |
| `cargo-deny` bans / sources | wildcard versions and non-crates.io (supply-chain) deps |
| provenance gate | an adapter without a clean-room `PROVENANCE.md` |
| trademark gate | a vendor mark leaking into package identity |
| fmt · clippy · check · docs | formatting, lints, compilation, doc warnings |
