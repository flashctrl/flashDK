# Security Policy

## Reporting a vulnerability

Please report security issues privately via GitHub Security Advisories
("Report a vulnerability" on the Security tab) rather than a public issue.

We aim to acknowledge reports within a few days.

## Scope

flashDK is a client library. It talks to IP-KVM devices over the network using each
device's own protocol. Note that transport security depends on the device: some (e.g.
PiKVM) offer TLS the client can pin; others serve cleartext HTTP on the LAN. flashDK
surfaces each device's real security posture rather than masking it — see the
`tls_pinnable` capability.

## Supply chain

Dependencies are gated in CI by `cargo-deny` (licenses, RustSec advisories, bans, and
source integrity). See `deny.toml`.
