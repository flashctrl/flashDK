# Provenance: nanokvm adapter

Per CLEANROOM.md, this adapter is implemented from **wire observation and official
documentation only**. No source code from any third-party project or SDK was read or copied.

## Sources
- Live off-the-wire probing of a physically-owned device (HTTP requests/responses,
  WebSocket frames, and/or WebRTC signaling), captured 2026-08-20.
- Official vendor documentation (public web docs/wiki).
- Public standards: USB HID Usage Tables, WebRTC/ICE/DTLS, RTP, MJPEG, JWT.

## Capture evidence
- See ../../../../docs/captures/ for raw request/response logs backing each mapped endpoint.

## Attestation
No GPL/copyleft source (kvmd, NanoKVM, JetKVM, GL.iNet firmware) or any SDK was consulted
to write this adapter. Interface facts (endpoint paths, field names, RPC method names) were
recorded from the wire, not from source.

## Quarantine notice (2026-11-12): password login removed

An earlier version of this adapter implemented the web client's password login using a
passphrase described in the code as "baked into NanoKVM's frontend bundle". That
passphrase appears only inside the vendor's web client, never on the wire, so this
adapter could not have derived it from permitted sources. It breaches the rule in
CLEANROOM.md against lifting protocol constants out of source. The login code and its
dependencies are removed from the current tree.

What was tried, all against an owned device, to find a permitted source:
- Logging in with a plain password: rejected (`code -2`, same as a wrong password).
- HTTP Basic authentication on an authenticated endpoint: rejected (`401`).
- TLS: no HTTPS listener on the tested unit (port 443 closed).
- Sipeed's wiki: documents default credentials and password resets, and no API login.

Not yet resolved:
- The removed code, and the commit that introduced it, remain in git history. Whether to
  rewrite published history is a maintainer decision, ideally with a licensing lawyer.
- Restoring password login needs the vendor to document it, or to permit it in writing.

Supported today: `NanoKvm::connect_with_token`, which takes a session token the user
obtained by signing in through the device's own web interface. The adapter never sees
the password and contains no login cryptography. The HID WebSocket protocol is
unaffected and remains derived from wire observation (docs/captures/nanokvm-ws-hid.md).
