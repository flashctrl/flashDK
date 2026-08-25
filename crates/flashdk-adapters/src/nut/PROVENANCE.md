# Provenance: nut adapter

Per CLEANROOM.md, this adapter is implemented from public standards and official
documentation only. No NUT source code (`github.com/networkupstools/nut`) was read
or copied.

## Sources

- **RFC 9271** ("Uninterruptible Power Supply (UPS) Management Protocol: Commands
  and Responses"), an IETF Informational RFC published by the NUT project
  community, https://www.rfc-editor.org/rfc/rfc9271.html. This is the primary
  source for the command grammar (`USERNAME`/`PASSWORD`/`GET VAR`/`LIST VAR`/
  `INSTCMD`), response framing (`OK`, `ERR <name>`, `BEGIN LIST .../END LIST ...`),
  the status-flag set, and the variable names `battery.charge` and `ups.status`
  and the instant command `test.panel.start`, each quoted directly from the RFC
  text where used in `protocol.rs`.
- Official NUT manual pages on `networkupstools.org` (e.g. `upscmd(8)`), consulted
  for command categories and the `load.off` example.
- The variable names `ups.load` and `battery.runtime`, and the instant commands
  `beeper.enable`/`beeper.disable`, are corroborated independently (community
  documentation and issue-tracker discussion, not NUT's own source) but are not
  directly quoted in the RFC text available to us. Marked as such, at
  lower confidence, in `protocol.rs`'s doc comments.

## Attestation

No source code from `networkupstools/nut` or any other third-party project was
read or copied to write this adapter.

## Verification status

The protocol encoder and parser (`protocol.rs`) are unit-tested against RFC
9271's own literal example text. The TCP client in `mod.rs` has **not** been
exercised against a running `upsd` server or a real UPS; this codebase has
neither available at the time of writing. See `docs/STATE.md`.
