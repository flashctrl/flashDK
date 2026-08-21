# Contributing to flashDK

Thanks for your interest! One rule matters more than any other here.

## The clean-room rule (non-negotiable)

This SDK is Apache-2.0 and must stay usable by proprietary clients. Every target
device's firmware is copyleft (GPL), so **all adapter code is implemented from wire
observation and official documentation only — never from any vendor's source or any
other SDK.** Read [CLEANROOM.md](CLEANROOM.md) in full before your first PR.

If you have read GPL source for a device you want to add support for, you are not
eligible to write that adapter. This protects the whole project's license.

## What CI enforces

Every PR runs:
- `cargo fmt --all --check` — formatting
- `cargo clippy -- -D warnings` — lints
- `cargo check` — it compiles
- `cargo deny check licenses` — **every dependency is permissively licensed** (the
  machine-enforced half of the clean-room rule; a GPL/LGPL crate fails the build)

Run these locally before pushing:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check --all-targets
```

## Adding a vendor adapter

1. Probe a device you physically own; capture the traffic into `docs/captures/`.
2. Implement the `flashdk-core` traits from those captures.
3. Add a `PROVENANCE.md` attesting wire-only sourcing.
4. Keep dependencies permissive (see `deny.toml`).
