# Provenance — nanokvm adapter

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
