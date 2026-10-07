# Provenance: jetkvm adapter

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

## Addendum 2026-10-07: virtual media over `rpc`
`listStorageFiles`, `getVirtualMediaState`, `mountWithStorage` (`mode: "CDROM"`) and
`unmountImage` were observed on the `rpc` DataChannel by wrapping the page's
`RTCDataChannel.send` while the official web client was used against an owned device.
See docs/captures/jetkvm-rpc-virtual-media.md. The device's frontend bundle was not
read. Only the CD/DVD storage-mount path is covered; URL mount, disk mode, and upload
are not captured. Power (ATX/DC extensions) remains uncaptured.

## Addendum 2026-10-07: WebSocket signaling (firmware 0.5.9)
`POST /webrtc/session` stopped working on firmware 0.5.9. The route
`/webrtc/signaling/client` was found by upgrade-probing the owned device and confirmed
by a `101` response; the device's first frame and its answer frame were recorded from
the wire. Candidate paths were guessed by the author, who cannot rule out having seen
JetKVM's public layout in training data; no JetKVM source was opened this session, and
the message format was not taken from any source (see
docs/captures/jetkvm-rpc-virtual-media.md for exactly what is observed versus inferred).
Upload and Disk-mode mount were observed from the official client's DataChannel/XHR
traffic, as above.
