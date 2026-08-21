# Capture — JetKVM WebRTC DataChannel HID (wire-observed)

Device: JetKVM, http://10.0.10.21. Control rides WebRTC DataChannels (not REST/JSON
over HTTP). Captured via an RTCDataChannel.send hook in the browser while trusted input
was generated. No vendor source read.

## DataChannels (client-created)
- `rpc`                          — JSON-RPC 2.0 control plane, e.g.
      {"jsonrpc":"2.0","method":"getLocalVersion","params":{},"id":"rpc_..._1"}
- `hidrpc`                        — BINARY HID (reliable) — keyboard + mouse
- `hidrpc-unreliable-ordered`     — BINARY HID (low-latency, ordered)
- `hidrpc-unreliable-nonordered`  — BINARY HID (low-latency)
- `terminal`, `serial`, `cdcacm` — console/serial functions

## HID binary frames (on hidrpc channels)

### Keyboard  — VERIFIED
`[0x05][usage][state]`  (3 bytes) — one event per key.
- usage: USB HID usage code; state: 0x01 down, 0x00 up.
- Observed: Esc down = `05 29 01`, Esc up = `05 29 00`.
- Maps directly to flashdk_core::hid::KeyEvent { key, pressed }.

### Mouse  — STRUCTURE SEEN, VALUES UNVERIFIED
Initial zero-state frames only (device had "No HDMI signal", so the client emitted no
movement frames — absolute positioning needs a visible screen):
- `hidrpc` type 0x02, 8 bytes: `02 00 00 00 00 00 00 00`  (likely absolute mouse)
- `hidrpc-unreliable-ordered` type 0x03, 10 bytes: `03 00 ...`  (likely relative/hi-rate)
- `hidrpc` `0101` (2 bytes) and `09` (1 byte) also seen — role TBD (LED/keepalive/sync).
Field layout must be confirmed with a signal-present capture on a NON-host target
(JetKVM's USB HID drives whatever it's plugged into — during capture that was the dev host).

## Transport note
Implementing this adapter requires a Rust WebRTC stack (webrtc-rs, BSD/MIT — clean):
HTTP signaling at /webrtc/session, ICE/DTLS, then open the `hidrpc` DataChannel and send
the binary frames above. This is the "PeerRpc" transport in flashdk_core.
