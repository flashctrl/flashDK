# Contributing to flashDK

Thanks for your interest. One rule matters more than any other here.

## The clean-room rule (non-negotiable)

This SDK is Apache-2.0 and must stay usable by proprietary clients. Every target
device's firmware is copyleft (GPL), so all adapter code is implemented from wire
observation and official documentation only, never from any vendor's source or any
other SDK. Read [CLEANROOM.md](CLEANROOM.md) in full before your first PR.

If you have read GPL source for a device you want to add support for, you are not
eligible to write that adapter. This protects the whole project's licence.

## What CI enforces

Every PR runs two jobs, both required to merge:

**Build, lint & docs:**
- `cargo fmt --all --check`: formatting
- `cargo clippy --all-targets --all-features -- -D warnings`: lints, warnings treated
  as errors
- `cargo check --all-targets`: it compiles
- `cargo doc --no-deps --all-features` with `RUSTDOCFLAGS=-D warnings`: doc comments
  build clean, including intra-doc links

**Licence, supply-chain & clean-room:**
- `cargo deny check licenses`: every dependency is permissively licensed, the
  machine-enforced half of the clean-room rule; a GPL/LGPL/AGPL crate fails the build
- `cargo deny check advisories`: no known-vulnerable or yanked dependency (see
  `deny.toml` for the one documented exception and why)
- `cargo deny check bans / sources`: no wildcard version requirements, no
  non-crates.io dependency sources
- A provenance check: every adapter under `crates/flashdk-adapters/src/` must carry
  a `PROVENANCE.md`
- A trademark check: no vendor name may appear as a package identity

Run the first set locally before pushing:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check --all-targets
cargo doc --no-deps --all-features
```

`cargo deny` needs installing separately (`cargo install cargo-deny`) if you want to
check the second set locally; otherwise CI runs it for you.

## Commit messages

Commits follow [Conventional Commits](https://www.conventionalcommits.org):
`type(scope): description`, lowercase, imperative mood, 72 characters or fewer. The
body says why, what it cost, and what you verified; the diff already says what
changed. See [docs/decisions.md](docs/decisions.md) for why this format was chosen
over the alternative.

A hook checks the format automatically. Enable it once per clone:

```bash
git config core.hooksPath .githooks
git config commit.template .gitmessage
```

Bypass it for a single commit with `git commit --no-verify` if you have a genuine
reason.

## Adding a vendor adapter

1. Probe a device you physically own; capture the traffic into `docs/captures/`.
2. Implement the `flashdk-core` traits from those captures.
3. Add a `PROVENANCE.md` attesting wire-only sourcing.
4. Keep dependencies permissive (see `deny.toml`), and record any new one in
   [docs/credits.md](docs/credits.md).
