# Decisions

flashDK's own record. Numbered, stable, never reused. A reversed decision gets a new
entry; the old one stays in place.

## 1. Rust core with UniFFI bindings, over Kotlin Multiplatform or two native codebases

The maintainer is experienced with computers generally but not with programming
specifically, and asked for a recommendation rather than having a preference of
their own.

**Chosen:** a single Rust core, with UniFFI generating Swift and Kotlin bindings
for the native apps.

**Why:** one implementation of the protocol logic, which is where the actual bugs
live, rather than two. Rust also lets non-Rust consumers besides the mobile apps
(a CLI, a desktop tool) use the same crate.

**What it cost:** a Rust learning curve for the maintainer, and Swift/UniFFI interop
has rough edges that a hand-written Swift client wouldn't have.

**Rejected:** Kotlin Multiplatform (weaker Swift interop story than advertised);
two separate native codebases (every protocol fix done twice, and the two
implementations drift); a TypeScript core with React Native or WebView shells (not
a genuinely native feel, and WebRTC performance on mobile is the weak point for a
KVM client specifically).

**What would justify revisiting:** if UniFFI's Swift bindings prove unworkable in
practice once a real iOS app is built against them.

## 2. The SDK is Apache-2.0, kept legally separate from any GPL firmware work

**Chosen:** flashDK is permissively licensed and usable by proprietary clients
without restriction. A separate, GPL-licensed firmware fork (tracked in its own
repository, not this one) stays entirely out of this codebase.

**Why:** the maintainer wants a proprietary mobile app to be the flagship client,
while the protocol layer itself is open enough that competitors can build on it. A
GPL SDK would force the proprietary app open the moment it linked against it.

**What it cost:** a competitor is free to build a rival app on the same SDK. That
is treated as the price of openness being real, not a mistake.

**Rejected:** a GPL SDK (defeats the proprietary app); copying any vendor source
regardless of the SDK's own declared licence, which would force copyleft onto it
independent of what the licence file says. See decision #3.

## 3. Every adapter is clean-room: wire and documentation only, never vendor source

**Chosen:** no adapter is written by reading a vendor's source code, decompiling a
binary, or consulting a community write-up that itself copied from source. See
[CLEANROOM.md](../CLEANROOM.md) for the full charter.

**Why:** all four target vendors' firmware is copyleft (PiKVM and NanoKVM GPLv3,
JetKVM GPL-2.0, GL.iNet unverified but presumed similar). Reading their source to
write an adapter risks a derivative-work claim that would force copyleft onto this
SDK regardless of decision #2.

**What it cost:** real facts stay unknown for longer than they would if source were
consulted. JetKVM's power and virtual-media method names, for instance, are visible
in the vendor's minified frontend bundle right now and deliberately left uncaptured
from that source; see [STATE.md](STATE.md).

**What would justify revisiting:** nothing. This is the load-bearing constraint the
whole SDK depends on, not a cost/benefit call.

## 4. `str0m` over `webrtc-rs` for JetKVM's WebRTC transport

**Chosen:** `str0m`, a sans-IO WebRTC implementation.

**Why:** `str0m`'s dependency tree pulls no `ring`, whereas `webrtc-rs`'s common
configuration does. `ring`'s licence field is a non-standard SPDX expression that
complicates automated licence checking (see decision #5 for the same
consideration applied to the TLS side).

**What it cost:** `str0m` is sans-IO, so this project owns the UDP socket, the
timer loop, and the event dispatch that `webrtc-rs` would otherwise provide at a
higher level. That is real, hand-written transport code
(`crates/flashdk-adapters/src/jetkvm/transport.rs`) that a higher-level library
would have avoided.

**Rejected:** `webrtc-rs` (would need a written, ongoing exception in `deny.toml`
for `ring`'s licence, rather than a clean pass).

**What would justify revisiting:** if `str0m` proves unable to interoperate with
a future vendor's WebRTC implementation, where `webrtc-rs`'s broader compatibility
testing might succeed where `str0m` doesn't.

## 5. `aws-lc-rs`, not `ring`, as the TLS crypto provider

**Chosen:** PiKVM's trust-on-first-use certificate pinning (decision #7) is built
on `rustls` with `aws-lc-rs` supplying the cryptographic primitives.

**Why:** consistent with decision #4. `aws-lc-sys`'s licence field is a compound
expression of standard SPDX identifiers (Apache-2.0, ISC, MIT, BSD-3-Clause), every
one of which is already on the project's allow list in `deny.toml`, so adopting it
required no configuration change at all.

**What it cost:** `aws-lc-sys` compiles C and assembly from source on first build,
which took roughly five minutes in CI the first time this was added. `ring` ships
more build-friendly precompiled paths for common targets, and has a longer track
record on mobile cross-compilation, which this project will need once it targets
iOS and Android. That mobile-specific friction hasn't been hit yet and is flagged
here rather than discovered later without a record of why the trade was made.

**Rejected:** `ring` (the licence-expression complication described above; not
strictly unresolvable, just avoided).

**What would justify revisiting:** a real, blocking cross-compilation failure
against an iOS or Android target that `ring` would not have hit.

## 6. `reqwest` uses `rustls` only; `native-tls`/OpenSSL is dropped entirely

**Chosen:** the `flashdk-adapters` crate builds `reqwest` with
`default-features = false` and the `rustls-tls-manual-roots-no-provider` feature,
rather than the default `native-tls` backend.

**Why:** NanoKVM and JetKVM only ever speak plain `http://`, so no TLS backend is
invoked for them regardless of what's compiled in. PiKVM's HTTPS goes through a
hand-built `rustls::ClientConfig` for certificate pinning (decision #7) anyway.
`native-tls` was providing nothing.

**What it cost:** nothing identified so far. The benefit is a smaller dependency
tree (no OpenSSL binding at all) and one fewer TLS stack to audit or cross-compile
for mobile later.

**What would justify revisiting:** a future adapter that genuinely needs
`native-tls`'s platform-specific certificate store integration (Keychain,
Windows CertStore) in a way `rustls` can't match.

## 7. Trust-on-first-use certificate pinning, not blanket certificate acceptance

**Chosen:** PiKVM's adapter pins the SHA-256 fingerprint of the certificate seen on
first connection and rejects a different one later, rather than accepting any
certificate from anyone.

**Why:** PiKVM ships a self-signed certificate, so ordinary certificate-authority
validation can never succeed. The previous approach,
`danger_accept_invalid_certs(true)`, accepted literally any certificate from any
party, which provides no protection against interception at all. See
[security.md](security.md) for the full reasoning.

**What it cost:** real implementation complexity, a custom `rustls`
`ServerCertVerifier`, and a new dependency on `sha2`. It also introduces a genuine
usability cost that a user will eventually hit: a legitimate device re-key (the
device's certificate regenerated after a firmware update or factory reset) is
indistinguishable, from the pin's point of view, from an actual attacker. An app
built on this SDK needs to surface that distinction to a human rather than silently
failing or silently re-pinning.

**Rejected:** continuing to accept any certificate (no protection); bundling a
fixed allow-list of expected certificates (doesn't work, since every device
generates its own unique self-signed certificate at first boot).

**What would justify revisiting:** nothing about the mechanism. The open question
is how a consuming app should present a pin mismatch to a user, which belongs to
the app, not the SDK.

## 8. `Cargo.lock` is committed, departing from typical Rust library convention

**Chosen:** `Cargo.lock` is tracked in git, not gitignored.

**Why:** the usual advice for a Rust *library* crate is to omit the lockfile, so a
downstream consumer's own resolution controls the final dependency versions. This
workspace, though, also ships runnable example binaries
(`crates/flashdk-adapters/examples/`) that are built and run directly, in CI and by
contributors, and an uncommitted lockfile meant every fresh clone or CI run could
silently resolve different transitive dependency versions over time. Note that
this decision affects only the examples and CI: a project that depends on
`flashdk-core` or `flashdk-adapters` as a library dependency resolves its own
versions regardless of what this repository's lockfile says.

**What it cost:** the lockfile needs reviewing (or at least skimming) on dependency
bumps, since it's now a visible part of every diff that touches `Cargo.toml`.

**What would justify revisiting:** if the crates are split into their own
publishable-only repository with no example binaries, the original library
convention would apply cleanly again.

## 9. Conventional Commits, adopted going forward, not applied to existing history

**Chosen:** commits from this point forward follow
[Conventional Commits](https://www.conventionalcommits.org), enforced by a
commit-msg hook. Commits before this decision are left exactly as they are.

**Why:** a machine-readable commit format unlocks changelog generation and
semantic versioning later at no ongoing cost, and searchable history
(`git log --grep '^fix'`) is useful immediately. Rewriting existing history to
match would destroy `git blame` for a purely cosmetic gain, which is a bad trade on
its own terms regardless of the format's merits.

**What it cost:** `git log --oneline` shows two distinct styles across the
project's life, permanently. That's treated as an honest record of when the
convention was adopted, not something to paper over.

**Rejected:** rewriting history to retroactively conform (destroys blame for no
functional benefit); leaving commit format entirely unstandardized going forward
(loses the machine-readability benefit for free).

**What would justify revisiting:** nothing. This is the intended permanent state,
not a placeholder.
