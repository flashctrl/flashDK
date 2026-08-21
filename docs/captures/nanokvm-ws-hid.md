# Capture — NanoKVM /api/ws binary HID protocol (wire-observed)

Device: NanoKVM PCIe, app 2.5.0, 10.0.10.10. WebSocket: `ws://<host>/api/ws`
(also available over wss/443). Auth: `nano-kvm-token` cookie (JWT from AES login).

Method: a generic `WebSocket.send` hook installed in the browser recorded the exact
bytes the official web client sends, while real (trusted) pointer/keyboard input was
generated. No vendor source was read — only the frames on the wire.

## Frame types (client -> server)

### Heartbeat
`00`  (single byte, ~every 10s)

### Keyboard  (9 bytes) — standard USB HID keyboard report with a 1-byte type prefix
`[0x01][modifiers][0x00][k1][k2][k3][k4][k5][k6]`
- modifiers: USB HID modifier bitmask (bit0 LCtrl,1 LShift,2 LAlt,3 LGUI,4 RCtrl,5 RShift,6 RAlt,7 RGUI)
- k1..k6: up to six simultaneous USB HID usage codes (0x00 = empty)
- Observed: Esc down = `01 00 00 29 00 00 00 00 00` (0x29 = Esc);
            release  = `01 00 00 00 00 00 00 00 00`

### Mouse, absolute  (7 bytes)
`[0x02][buttons][Xlo][Xhi][Ylo][Yhi][wheel]`
- buttons: bitmask bit0 left, bit1 right, bit2 middle
- X,Y: little-endian u16, 0..32767 = fraction of screen (0 = left/top, 32767 = right/bottom)
- wheel: signed i8
- Calibration (fraction -> value): 0.25->8167, 0.50->16333/16414, 0.75->24499/24554
  (i.e. value ~= fraction * 32767)
- Observed left-button press = byte1 0x01; release = 0x00.

Note: only absolute mouse (type 0x02) was observed. A relative-mouse frame type, if any,
is not yet characterized.

## REST endpoints (aux HID, power, storage) — observed contracts

- GET  /api/vm/gpio                 -> {pwr, hdd}  (power/activity state)
- POST /api/vm/gpio {type, duration} -> hold power/reset line for `duration` ms.
  Valid `type` values: "power" and "reset" (confirmed via duration:0 probes, which
  validate the name without actuating; all other candidates return "invalid power event").
  Short tap vs. ~5s long-press (force off) selected by duration.
- GET  /api/storage/image           -> {files: [...]|null}  (available images)
- GET  /api/storage/image/mounted   -> {file: ""}           (currently mounted, "" = none)
- POST /api/storage/image/mount {file: name} mounts; {file: ""} unmounts.
