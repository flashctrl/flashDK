# flashDK

[![CI](https://github.com/flashctrl/flashDK/actions/workflows/ci.yml/badge.svg)](https://github.com/flashctrl/flashDK/actions/workflows/ci.yml) [![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

**Status: in testing.** Core HID (keyboard and mouse) is implemented and verified
live against real hardware for PiKVM, NanoKVM, and JetKVM. Power control and virtual
media are implemented and verified for PiKVM and NanoKVM; JetKVM's equivalents ride a
separate JSON-RPC channel that isn't wired up yet. GL.iNet has no adapter, because
the project doesn't own the hardware to probe it against. See
[docs/STATE.md](docs/STATE.md) for the current picture in full, including what still
needs a real device to finish.

**LLM use:** this project is built with substantial AI assistance (Claude Code,
Anthropic). Every adapter is derived from live wire captures the model performed
against real, owned devices, with a clean-room boundary against reading any vendor's
source (see [CLEANROOM.md](CLEANROOM.md)). A human directs the work, reviews the
diffs, and owns the licensing and security decisions; the model does the
protocol-level implementation and verification.

A clean-room, **Apache-2.0** SDK for controlling IP-KVM devices across vendors
(Sipeed NanoKVM, PiKVM, JetKVM; GL.iNet planned). One protocol implementation, meant
to be embedded in native apps via UniFFI (Swift/Kotlin) and freely usable by any
client, proprietary or open.

> Built entirely from wire observation and official docs, never from anyone's
> source. See [CLEANROOM.md](CLEANROOM.md).

## Layout

```
crates/
  flashdk-core/       # vendor-neutral model: traits + types, zero dependencies
    src/
      capability.rs     # Capabilities (what a device supports) + Vendor
      transport.rs      # TransportKind: REST/WS vs WebRTC-DataChannel-RPC (key split)
      hid.rs            # keyboard/mouse contract, the layer that unifies cleanly
      power.rs          # power actions/state
      media.rs          # virtual media (mount ISO/image)
      device.rs         # Device umbrella trait + DeviceInfo
      error.rs          # one Error type
  flashdk-adapters/   # one module per vendor, implements the core traits
    src/
      nanokvm/  pikvm/  jetkvm/     (+ PROVENANCE.md each)
      tls_pin.rs        # shared trust-on-first-use TLS pinning (PiKVM today)
      lib.rs            # `Kvm` enum: runtime dispatch across vendors, no `dyn`
docs/captures/          # raw wire-capture evidence (the clean-room audit trail)
```

## Reading order (for learning)

`flashdk-core/src/lib.rs` → `capability.rs` → `transport.rs` → `hid.rs` →
`device.rs`, then `flashdk-adapters/src/lib.rs` and one adapter. The doc-comments
(`//!` and `///`) explain the why, not just the what. For the project's own
reasoning and history, see [docs/README.md](docs/README.md).

## Build

```bash
# Install Rust once, if you haven't:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Then, from this directory:
cargo check     # type-check the whole workspace
cargo doc --open --no-deps   # read the annotated API in your browser
```

## What comes next

- JetKVM power and virtual media, over its `rpc` JSON-RPC channel (the transport is
  live; only the method names and payload shapes still need a clean wire capture)
- A GL.iNet adapter, once a unit is available to probe against
- A UniFFI layer generating Swift/Kotlin bindings from these same traits
- TOFU certificate pinning extended to future TLS-capable adapters (already shipped
  for PiKVM; see [docs/security.md](docs/security.md))

## Trademarks and interoperability

flashDK is an **independent, clean-room interoperability library**. It is not
affiliated with, endorsed by, or sponsored by any device vendor.

PiKVM, NanoKVM, Sipeed, JetKVM, and GL.iNet are trademarks of their respective
owners. Those names appear here only nominatively, to state which devices flashDK
can talk to. They are never used as flashDK's own branding or package identity, and
CI enforces that (see `.github/workflows/ci.yml`).

Every adapter is implemented solely from observed network behaviour and official
public documentation, never from a vendor's source code. See
[CLEANROOM.md](CLEANROOM.md) and each adapter's `PROVENANCE.md`. This is what keeps
the SDK licensable under Apache-2.0, independent of the vendors' own copyleft
firmware licenses.

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
