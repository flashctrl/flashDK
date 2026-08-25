# Capture: JetKVM WebRTC DataChannel HID (wire-observed)

Device: JetKVM, http://10.0.10.21. Control rides WebRTC DataChannels, not REST/JSON
over HTTP. Captured via an RTCDataChannel.send hook in the browser while trusted
input was generated. No vendor source read.

## DataChannels (client-created)

- `rpc`: JSON-RPC 2.0 control plane, e.g.
  `{"jsonrpc":"2.0","method":"getLocalVersion","params":{},"id":"rpc_..._1"}`
- `hidrpc`: binary HID (reliable), keyboard and mouse
- `hidrpc-unreliable-ordered`: binary HID (low-latency, ordered)
- `hidrpc-unreliable-nonordered`: binary HID (low-latency)
- `terminal`, `serial`, `cdcacm`: console/serial functions

## HID binary frames (on hidrpc channels)

### Keyboard: verified

`[0x05][usage][state]` (3 bytes), one event per key.
- usage: USB HID usage code; state: 0x01 down, 0x00 up.
- Observed: Esc down = `05 29 01`, Esc up = `05 29 00`.
- Maps directly to flashdk_core::hid::KeyEvent { key, pressed }.

### Mouse, absolute: verified (on `hidrpc-unreliable-ordered`, type 0x03, 10 bytes)

`[0x03][buttons][X 24-bit BE (idx2..4)][pad idx5][Y 24-bit BE (idx6..8)][wheel idx9]`
- X, Y: 0..32767 = fraction of the target screen (big-endian). Matches core AbsMouse.
- buttons: bitmask at idx1 (bit0 left, etc.). wheel: signed at idx9.
- Verified with calibrated hovers on a Proxmox console (1280x720):
    fx0.90/fy0.50 -> `03 00 00 7308 00 00 4000 00` (X=29448, Y=16384)
    fx0.11/fy1.00 -> `03 00 00 0e58 00 00 7fff 00` (X=3672, Y=32767)
- `hidrpc` type 0x02 (8 bytes) stays zero during moves, likely the reliable/on-change
  absolute report or button channel; `0101`/`09` are keepalive/sync. The relative-mouse
  frame type is not characterized (the client used absolute throughout).

## Transport

Implemented on `str0m` (sans-IO WebRTC, MIT), not the `webrtc-rs`/BSD approach
originally sketched here; see [decisions.md](../decisions.md) #4 for why. Signaling
happens at `/webrtc/session`, then ICE and DTLS, then the `hidrpc` DataChannel opens
and carries the binary frames above. This is the `PeerRpc` transport in
`flashdk_core`, implemented in `crates/flashdk-adapters/src/jetkvm/transport.rs`.

## System architecture (SSH-observed, system facts only, no source read)

Device: Linux 5.10.160 armv7l (busybox). Observed via root SSH:
- Process `jetkvm [app]` listens on :80 (web UI, control, WebRTC signaling).
- Process `jetkvm [native]` listens on 127.0.0.1:3893 (local media/HID engine).
- `dropbear` on :22.
- Composite USB gadget 0x1d6b/0x0104: hid.usb0 (keyboard, 8B report),
  hid.usb1 (mouse, 6B), hid.usb2 (mouse, 5B), hid.usb3 (1B consumer/control),
  mass_storage.usb0, uac1.usb0 (audio).
- The compact hidrpc keyboard event [0x05][usage][state] maps onto the 8B USB
  keyboard report; mouse hidrpc frames (types 0x02/0x03) map onto the mouse HID
  interfaces. The hidrpc *wire* format comes from DataChannel capture, not device
  source.

## Signaling: probed, then finalized live

Browser interception was not possible: the web client captures
window.fetch/WebSocket/RTCPeerConnection at bundle-eval time, so post-load hooks are
bypassed, the device has no packet-capture tools, and the dev host can't run tcpdump
without root. The exact signaling contract was instead inferred by black-box probing
`POST /webrtc/session` (authenticated):
- `{}` (application/json) -> `{"error":{"Offset":0}}`
- `{"sd":"test"}` -> `{"error":{"Offset":1}}`
- a JSON *string* body (quoted base64) -> `{"error":"Invalid request body"}` (400)

The Offset errors are Go `json.Unmarshal`-into-string failures, meaning the body is
expected to be a JSON string whose value is base64. That hypothesis was then
confirmed and finalized by generating a real SDP offer with `str0m`, wrapping it as
`{"sd": base64(JSON {type,sdp})}`, and iterating the exact wrapper against the live
device until it returned a valid answer in the same shape. No device source was
consulted at any point. The working implementation is
`crates/flashdk-adapters/src/jetkvm/transport.rs::exchange_sdp`.

## rpc control channel (JSON-RPC 2.0): verified

The `rpc` DataChannel carries JSON-RPC 2.0 as text. Request/response correlation by
`id` works over `str0m`. Verified live: `getLocalVersion` (no params) returns
`{"appVersion":"0.5.9-dev...","systemVersion":"0.2.8"}`. Power and virtual-media
method names and payload shapes still need to be captured off this same channel
before they can be wired up; see [STATE.md](../STATE.md).
