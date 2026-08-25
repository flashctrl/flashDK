# Credits

What flashDK depends on, what each is used for, and its licence. Licences marked
`VERIFY` have not been checked against the crate's actual manifest. Do that before
distributing this repo, or any binary built from it, commercially.

```bash
grep -rn "VERIFY" docs/
```

This table lists direct dependencies only, read from each crate's own `Cargo.toml`
`license` field at the version currently pinned in `Cargo.lock`, not guessed from
the crate's reputation or its README.

## `flashdk-core`

No dependencies. Kept that way deliberately: the vendor-neutral model needs
nothing but the standard library, so there is nothing here to audit.

## `flashdk-adapters`

| Crate | Used for | Licence |
|---|---|---|
| `flashdk-core` | The workspace's own vendor-neutral crate | Apache-2.0 |
| `reqwest` | HTTP client (PiKVM, NanoKVM, JetKVM signaling) | MIT OR Apache-2.0 |
| `serde` / `serde_json` | Wire-format (de)serialization | MIT OR Apache-2.0 |
| `rustls` | TLS, underlying PiKVM's trust-on-first-use pinning | Apache-2.0 OR ISC OR MIT |
| `aws-lc-rs` | `rustls`'s cryptographic provider | ISC AND (Apache-2.0 OR ISC) |
| `aws-lc-sys` | `aws-lc-rs`'s native bindings | ISC AND (Apache-2.0 OR ISC) AND Apache-2.0 AND MIT AND BSD-3-Clause AND (Apache-2.0 OR ISC OR MIT) AND (Apache-2.0 OR ISC OR MIT-0) |
| `sha2` | Certificate fingerprinting for pinning | MIT OR Apache-2.0 |
| `tokio` | Async runtime | MIT |
| `tokio-tungstenite` | WebSocket client (NanoKVM's HID transport) | MIT |
| `futures-util` | Stream/sink combinators for the WebSocket split | MIT OR Apache-2.0 |
| `str0m` | Sans-IO WebRTC (JetKVM's HID transport) | MIT OR Apache-2.0 |
| `aes` / `cbc` | AES-CBC, replicating NanoKVM's login encryption | MIT OR Apache-2.0 |
| `md-5` | Key derivation for the same NanoKVM login scheme | MIT OR Apache-2.0 |
| `base64` | Encoding for NanoKVM's login and JetKVM's signaling | MIT OR Apache-2.0 |
| `rand` | Salt generation for the NanoKVM login scheme | MIT OR Apache-2.0 |

Every licence above is permissive and compatible with this SDK's own Apache-2.0
licensing and with embedding in a proprietary client. `deny.toml`'s allow list is
the machine-enforced version of this table; a dependency whose licence isn't on
that list fails CI before it fails a human review.

The `aws-lc-sys` compound expression looks alarming at a glance because of its
length, but every atom in it (`Apache-2.0`, `ISC`, `MIT`, `BSD-3-Clause`, `MIT-0`)
is independently permissive; see [decisions.md](decisions.md) #5 for why this
crate was chosen over the alternative with a simpler-looking but non-standard
licence field.

## Evaluated and not adopted

No criticism is intended of any of these; each is a reasonable choice for a
different set of constraints than this project has.

| Considered | For | Outcome |
|---|---|---|
| `webrtc-rs` | JetKVM's WebRTC transport | Rejected in favour of `str0m`. See [decisions.md](decisions.md) #4. |
| `ring` | Cryptographic provider for `rustls` | Rejected in favour of `aws-lc-rs`. See [decisions.md](decisions.md) #5. |
| `native-tls` | HTTP client's TLS backend | Dropped entirely; `rustls` covers the one adapter (PiKVM) that needs TLS at all. See [decisions.md](decisions.md) #6. |

## flashDK's own licence

Apache-2.0, chosen deliberately so the SDK stays freely usable by proprietary
clients built on top of it. See [decisions.md](decisions.md) #2 for why, and why
this is kept legally separate from any GPL-licensed firmware work.
