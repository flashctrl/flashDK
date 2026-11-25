# flashDK

**An Apache-2.0 Rust SDK that gives apps one control interface for IP-KVM devices from different vendors.**

> **Status: in testing.** Nothing is tagged or published, and several features are
> unverified on real hardware (see [docs/STATE.md](docs/STATE.md)).
> This project comes with no warranty or guarantee of fitness for any purpose. Use it
> at your own risk.
>
> **LLM use.** flashDK is built with substantial AI assistance (Claude Code,
> Anthropic). Adapters come from wire captures the model performed against real devices
> the maintainer owns, and from official documentation. A human directs the work,
> reviews changes, and owns the licensing and security decisions.

[![CI](https://github.com/flashctrl/flashDK/actions/workflows/ci.yml/badge.svg)](https://github.com/flashctrl/flashDK/actions/workflows/ci.yml) [![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

## What it does

flashDK takes a device address and credentials and returns a handle with keyboard,
mouse, power and virtual-media operations. The same calls work across vendors, and each
device reports which of them it supports and whether its connection is encrypted. Rust
code uses the crates directly. Android and iOS apps are meant to use the UniFFI layer
in `crates/flashdk-ffi`.

Adapters exist for Sipeed NanoKVM, PiKVM, JetKVM and GL.iNet. There is also early,
hardware-unverified work for Redfish, Intel AMT (protocol layer only) and the NUT UPS
protocol.

## Use cases

**Reach for it when** you are writing a client that has to drive more than one brand of
KVM, and you need a licence that lets you ship it inside a proprietary app. The vendors'
own firmware is often copyleft, so flashDK is written from observed network behaviour
and public documentation instead of their source (see [CLEANROOM.md](CLEANROOM.md)).

## Getting started

Install Rust with [rustup](https://rustup.rs), then run these from the repository root.

```bash
cargo test --workspace
```

```bash
cargo doc --open --no-deps
```

To try an adapter against a device you own, use its example under
`crates/flashdk-adapters/examples/`. Each file's header says which environment
variables it reads and whether it sends input or presses a power button.

## Usage

`connect` takes a `TlsPolicy`. `RequireTls` is the default choice and never sends
credentials in cleartext. `PreferTls` falls back to cleartext only when nothing answers
on the HTTPS port, and never after a certificate failure or for a device whose
certificate is already pinned. `CleartextOnly` skips TLS. NanoKVM and JetKVM ship with
HTTPS off, so a stock unit needs one of the last two. PiKVM and GL.iNet are HTTPS only.

Certificates are pinned on first use. If a device later shows a different certificate,
the connect fails with `CertificateChanged`, which carries the new certificate's
fingerprint. It can mean an interception, so show the fingerprint to the user. Only if
they accept, save that exact fingerprint as the new pin and reconnect. Deleting the old
pin instead would trust whatever certificate answers next. Give the SDK a persistent pin store, or every
launch trusts whatever certificate it sees first.

NanoKVM password login is not available. It depended on a secret that only exists in
the vendor's web client, so it was removed (see
[PROVENANCE.md](crates/flashdk-adapters/src/nanokvm/PROVENANCE.md)). Sign in through
the device's own web page and pass the session token to
`NanoKvm::connect_with_token`, or `connectNanokvmWithToken` from Kotlin.

Kotlin bindings are generated with the `uniffi-bindgen` binary in `flashdk-ffi`, built
with the `bindgen` feature. Swift generation has not been tried yet.

## How it works

`flashdk-core` holds the vendor-neutral traits and types and has no dependencies.
`flashdk-adapters` has one module per vendor and the shared TLS pinning. `flashdk-ffi`
wraps both for Kotlin and Swift. [docs/architecture.md](docs/architecture.md) explains
the split.

## Documentation

[docs/README.md](docs/README.md) indexes the project's records. Start with
[STATE.md](docs/STATE.md) for what is verified,
[decisions.md](docs/decisions.md) for the reasoning and
[security.md](docs/security.md) for the trust model. `docs/captures/` holds the wire
evidence behind each adapter.

## Development

Run the checks CI runs before you push: `cargo fmt --all --check`,
`cargo clippy --all-targets --all-features -- -D warnings`,
`cargo test --workspace`, and `cargo doc --no-deps --all-features` with
`RUSTDOCFLAGS="-D warnings"`. CI also runs `cargo deny`, checks that every adapter has
a `PROVENANCE.md`, keeps vendor trademarks out of package names, cross-compiles the
FFI for Android, and holds new dependency versions to a release-age cooldown
(`scripts/check-release-age.sh`). See [CONTRIBUTING.md](CONTRIBUTING.md).

PiKVM, NanoKVM, Sipeed, JetKVM and GL.iNet are trademarks of their owners. flashDK is
independent and is not affiliated with or endorsed by any of them. Their names appear
only to say which devices the library talks to.

## Licence

Apache-2.0, see [LICENSE](LICENSE). Third-party licences that need a note are in
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md), and dependencies are listed in
[docs/credits.md](docs/credits.md).
