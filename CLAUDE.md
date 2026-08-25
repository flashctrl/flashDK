# Working agreement for AI agents in this repo

Repo-specific conventions. For the general standard this follows, see
[conformIT](https://github.com/almadon/conformIT); for why a given decision was
made, see [docs/decisions.md](docs/decisions.md), not here.

## The one rule that overrides everything else

Read [CLEANROOM.md](CLEANROOM.md) before writing a single line in
`crates/flashdk-adapters/`. Every adapter is implemented from wire observation and
official documentation only. Never read a vendor's source code, a decompiled
binary, or a community write-up that copied from either, even to "just understand"
something. This is not a style preference; it is the constraint that keeps the
whole SDK legally usable under Apache-2.0.

## Before every commit

```bash
. "$HOME/.cargo/env"    # only needed if cargo isn't already on PATH in this shell
cargo fmt --all
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo check --all-targets
cargo doc --no-deps --all-features         # RUSTDOCFLAGS=-D warnings for the CI-exact check
cargo test --workspace
```

All five must be clean before pushing. CI runs the same checks plus `cargo deny`
(licences, advisories, bans, sources), a provenance check, and a trademark check;
running the local subset first catches most failures before they cost a CI round
trip.

## Verify against real devices when you can, and say plainly when you can't

Every wire-protocol claim in this codebase should trace back to either a live
capture against a real, owned device, or a unit test pinned to bytes from one. If
a device is unreachable when you'd otherwise verify something live, say so
explicitly in the commit message rather than letting a compiled-and-untested
change read as verified. `docs/STATE.md` names outstanding unverified claims by
vendor.

## Credentials: never in code, never assumed safe in chat

No device password, token, or fingerprint belongs in a source file, a test
fixture using real captured values, or a commit. Every example reads its
credentials from environment variables at runtime. If a real device password
crosses a chat session during development, that session is not a secure channel
for it. See [docs/security.md](docs/security.md) for what happened the one time
that occurred here, and rotate the credential afterward rather than treating the
transcript as something that can be cleaned up after the fact.

## JetKVM specifically: mind what its USB is plugged into

JetKVM's USB HID gadget drives whatever machine it's physically connected to.
Sending HID input to a JetKVM that's plugged into the machine you're developing on
moves your own cursor and types into your own session, not into a remote target.
Verify JetKVM's HID adapter against a device plugged into something else, not the
development host.

## Where things go

Wire-format decoding and encoding belong in a vendor's `wire.rs` (or equivalent),
kept free of async networking code, so it can be unit-tested against literal
captured bytes without a live device or a network stack. A capture that informed
an adapter's behaviour gets recorded in `docs/captures/`, not just described in a
commit message. A decision worth defending later goes in
[docs/decisions.md](docs/decisions.md), with its cost and the alternative that
lost.
