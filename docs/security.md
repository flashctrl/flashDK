# Security

flashDK's own instance of a security posture: what the trust model actually is,
where credentials live, and what got traded away to get here. Not legal or
security advice; this is engineering reasoning specific to this project.

## Threat model

The devices this SDK talks to sit on a local network and hold real control over
physical machines: keyboard and mouse input, power, and in some cases boot media.
The threats that matter here are a network position that can intercept or inject
traffic to one of these devices, and a credential leaking somewhere it shouldn't.
This document does not cover a compromised development machine or a malicious
dependency that passes review; those are accepted risks at this project's current
scale, named here so the omission is deliberate rather than overlooked.

## Per-vendor transport security

The four target devices do not offer comparable security by default, and the SDK
is built to report that honestly rather than present one uniform "connected"
state.

| Vendor | Transport | What protects it |
|---|---|---|
| PiKVM | HTTPS | A self-signed certificate, pinned trust-on-first-use by this SDK (see below). Real transport encryption once pinned. |
| NanoKVM | Plain HTTP | Nothing. Credentials and HID input cross the network in the clear. The device's login flow obfuscates the password with a hardcoded, publicly-known AES key over that same unencrypted connection, which is not meaningful protection against anyone who can already observe the traffic. |
| JetKVM | HTTP signaling, then WebRTC | Signaling itself is unauthenticated by transport; the actual HID and video traffic is protected by WebRTC's own mandatory DTLS once the peer connection is established. |
| GL.iNet | Unknown | No adapter exists yet; nothing to state. |

`flashdk_core::Capabilities::tls_pinnable` reflects this directly: `true` for
PiKVM, `false` for NanoKVM and JetKVM. A consuming app should render this
difference visibly, a "hardened" indicator for a pinned connection and a
plaintext warning where there is none, rather than showing the same padlock icon
for all four vendors.

## Trust-on-first-use certificate pinning

PiKVM's certificate is pinned on first connection and any later connection
presenting a different certificate is rejected, rather than the SDK accepting any
certificate from any party. See [decisions.md](decisions.md) #7 for why this
replaced outright certificate-acceptance, and for the real trade-off it carries: a
legitimate device re-key looks identical to interception from the pin's point of
view. An app built on this SDK needs to surface that ambiguity to a person rather
than resolving it silently in either direction.

The verifier still performs full TLS signature verification
(`rustls::crypto::verify_tls12/13_signature`); only the certificate-authority
trust check is replaced with the pin comparison. A party presenting the pinned
certificate must still hold its private key.

## Credential handling in this codebase

No device credential is ever hardcoded in this repository. Every example binary
under `crates/flashdk-adapters/examples/` reads its host and credentials from
environment variables at runtime
(`PIKVM_HOST`/`PIKVM_USER`/`PIKVM_PASS` and equivalents), never from a literal in
source. Unit tests that need a JSON fixture resembling a real device's response
use placeholder values, not the actual data captured from a real device, even
where the real value isn't itself sensitive.

## A process finding, not a code finding

This is worth stating plainly rather than leaving implicit, because it's the kind
of thing that's easy to let slide during exploratory, device-in-hand development.

Building and verifying these adapters meant authenticating against real, owned
devices over a chat-based development session, and the credentials for those
devices were typed directly into that chat by the maintainer and used directly in
commands by the assistant. That means those plaintext passwords exist in the
session transcript for as long as the transcript or any summary of it is
retained, independent of anything in this git repository. Every commit in this
history has been checked for accidental credential inclusion, and none has been
found, but the chat transcript itself was never a clean channel for that
information in the first place.

The corrective action taken was rotating the affected device credentials once
active probing was complete, not attempting to scrub a conversational record
after the fact. Anyone extending this project the same way (a chat-driven
assistant with live access to real hardware) should treat "type the password
into the chat" as the wrong move from the start; a temporary credential, created
for the session and rotated out afterward, costs little and avoids the problem
entirely.

## Dependency and licence hygiene as a security practice

The clean-room requirement ([CLEANROOM.md](../CLEANROOM.md)) and the licence
allow-list enforced in CI (`deny.toml`) are framed elsewhere as a licensing
concern, but they double as a supply-chain one: every dependency's licence and
provenance is checked before it's added, and `cargo deny check advisories` fails
the build on a known-vulnerable or yanked crate. See [credits.md](credits.md) for
the current dependency list and what was evaluated and rejected.
