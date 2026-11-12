# Changelog

This file starts here, retroactively summarizing the project's history to date in
one entry rather than reconstructing per-commit entries after the fact. Entries
from this point forward are added alongside the change they describe, not
batched at release time.

## [Unreleased]

Nothing has been tagged or published yet; the workspace stays at `0.0.1`. Version
bumps and dated entries begin at the first real release.

### Security

- NanoKVM password login removed (quarantined): it relied on a secret taken from the vendor's web client, which CLEANROOM.md forbids. `NanoKvm::connect` now returns `NotSupported`; use `NanoKvm::connect_with_token`. See docs/decisions.md #10.

### Added

- The core/adapter workspace split (`flashdk-core`, `flashdk-adapters`), with the
  capability model, transport-kind distinction, and the vendor-neutral `Hid`,
  `Power`, `VirtualMedia`, and `Device` traits.
- A live, verified PiKVM adapter: keyboard and mouse over its REST HID API, power
  actions and state over `/api/atx`, virtual media over `/api/msd`.
- A live, verified NanoKVM adapter: session-token connection (password login was
  later removed, see Security), keyboard and mouse over its binary WebSocket protocol, power actions and
  state, and virtual media.
- A live, verified JetKVM adapter for keyboard and mouse, built on a sans-IO
  WebRTC transport (`str0m`) that this project wrote from scratch after
  determining the vendor's signaling contract by black-box iteration against a
  real device. JetKVM's power and virtual media are not implemented yet; see
  [docs/STATE.md](docs/STATE.md).
- Trust-on-first-use TLS certificate pinning (`tls_pin.rs`), replacing blanket
  certificate acceptance for PiKVM's self-signed certificate.
- A US-layout text-paste helper in `flashdk-core`, giving every adapter working
  `paste_text` through their existing `key()` implementation.
- CI enforcing formatting, lints, compilation, doc-comment correctness, licence
  compliance, dependency advisories, a clean-room provenance check per adapter,
  and a check against vendor trademarks appearing in package identity.
- The clean-room charter (`CLEANROOM.md`) governing how every adapter is sourced,
  and a `PROVENANCE.md` per adapter attesting to it.
