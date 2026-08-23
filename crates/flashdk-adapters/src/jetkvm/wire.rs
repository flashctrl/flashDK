//! JetKVM `hidrpc` binary HID frame encoding.
//!
//! Decoded from DataChannel captures (see docs/captures/jetkvm-datachannel-hid.md).
//! JetKVM carries HID as compact binary frames on its `hidrpc*` WebRTC DataChannels
//! (not JSON-RPC — that's the separate `rpc` channel for control). The unit tests
//! below assert the exact bytes observed from the official client.
//!
//! Frames:
//! * keyboard (3 bytes): `[0x05][usage][state]` — one event per key (not a full
//!   report), where `usage` is a USB HID usage code and `state` is 1 down / 0 up.
//!   This maps directly to `flashdk_core::hid::KeyEvent`.
//! * mouse, absolute (10 bytes): `[0x03][buttons][X big-endian][pad][Y big-endian]
//!   [wheel]`, X/Y in `0..=32767` = fraction of the screen — the same convention as
//!   `flashdk_core::hid::AbsMouse`. Sent on the `hidrpc-unreliable-ordered` channel.

// These encoders are decoded and verified ahead of the WebRTC transport that will call
// them; allow them to sit unused (outside tests) until that transport lands.
#![allow(dead_code)]

/// Encode a single key event. `usage` is a USB HID usage code; `pressed` = down.
pub fn key_event(usage: u8, pressed: bool) -> [u8; 3] {
    [0x05, usage, pressed as u8]
}

/// Encode an absolute-mouse frame. `x`/`y` are `0..=32767`; `buttons` is the bitmask
/// (bit0 left, bit1 right, bit2 middle); `wheel` is a signed tick.
///
/// The observed layout writes X and Y as big-endian with a zero high byte and a zero
/// separator (consistent with either 16-bit fields at \[3,4\]/\[7,8\] or 24-bit fields
/// at \[2,3,4\]/\[6,7,8\] — identical for our 0..=32767 range). We reproduce the exact
/// 10-byte frame.
pub fn mouse_abs(buttons: u8, x: u16, y: u16, wheel: i8) -> [u8; 10] {
    let [x_hi, x_lo] = x.to_be_bytes();
    let [y_hi, y_lo] = y.to_be_bytes();
    [
        0x03,
        buttons,
        0x00,
        x_hi,
        x_lo,
        0x00,
        0x00,
        y_hi,
        y_lo,
        wheel as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_matches_capture() {
        // Observed: Esc (0x29) down then up on the hidrpc channel.
        assert_eq!(key_event(0x29, true), [0x05, 0x29, 0x01]);
        assert_eq!(key_event(0x29, false), [0x05, 0x29, 0x00]);
    }

    #[test]
    fn mouse_matches_capture() {
        // Observed on hidrpc-unreliable-ordered during calibrated hovers:
        //   x=29448 (0.90), y=16384 (0.50) -> 03 00 00 7308 00 00 4000 00
        assert_eq!(
            mouse_abs(0, 29448, 16384, 0),
            [0x03, 0, 0x00, 0x73, 0x08, 0x00, 0x00, 0x40, 0x00, 0x00]
        );
        //   x=3672 (0.11), y=32767 (1.0)  -> 03 00 00 0e58 00 00 7fff 00
        assert_eq!(
            mouse_abs(0, 3672, 32767, 0),
            [0x03, 0, 0x00, 0x0e, 0x58, 0x00, 0x00, 0x7f, 0xff, 0x00]
        );
    }
}
