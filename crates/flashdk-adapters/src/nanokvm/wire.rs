//! NanoKVM `/api/ws` binary HID frame encoding.
//!
//! Decoded entirely from wire captures (see docs/captures/nanokvm-ws-hid.md); the
//! unit tests below assert the exact bytes observed from the official client, so the
//! encoders can't drift from the real protocol.
//!
//! Frames:
//! * keyboard (9 bytes): `[0x01][modifiers][0x00][k1..k6]` — a standard USB HID
//!   keyboard report; keys are raw HID usage codes, which is exactly what
//!   `flashdk_core::hid::KeyCode` carries.
//! * mouse (7 bytes): `[0x02][buttons][Xlo][Xhi][Ylo][Yhi][wheel]`, coords little-
//!   endian `0..=32767` — exactly `flashdk_core::hid::AbsMouse`'s convention.

/// USB HID modifier usage codes occupy 0xE0..=0xE7 and go in the report's modifier
/// bitmask rather than the key array.
const MOD_LO: u8 = 0xE0;
const MOD_HI: u8 = 0xE7;

/// The current set of held keys/modifiers, from which each keyboard report is built.
/// NanoKVM expects the *full* report every time, so the adapter tracks state and
/// re-sends the whole thing on each key event.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardState {
    mods: u8,
    keys: [u8; 6],
}

impl KeyboardState {
    /// Register a key press. Modifiers set their bit; regular keys fill the first
    /// free slot (ignored if already held or if all six slots are full).
    pub fn press(&mut self, usage: u8) {
        if (MOD_LO..=MOD_HI).contains(&usage) {
            self.mods |= 1 << (usage - MOD_LO);
        } else if !self.keys.contains(&usage) {
            if let Some(slot) = self.keys.iter_mut().find(|k| **k == 0) {
                *slot = usage;
            }
        }
    }

    /// Register a key release.
    pub fn release(&mut self, usage: u8) {
        if (MOD_LO..=MOD_HI).contains(&usage) {
            self.mods &= !(1 << (usage - MOD_LO));
        } else if let Some(slot) = self.keys.iter_mut().find(|k| **k == usage) {
            *slot = 0;
        }
    }

    /// The 9-byte keyboard frame for the current state.
    pub fn report(&self) -> [u8; 9] {
        [
            0x01,
            self.mods,
            0x00,
            self.keys[0],
            self.keys[1],
            self.keys[2],
            self.keys[3],
            self.keys[4],
            self.keys[5],
        ]
    }
}

/// Encode an absolute-mouse frame. `x`/`y` are `0..=32767`; `buttons` is the bitmask
/// (bit0 left, bit1 right, bit2 middle); `wheel` is a signed tick.
pub fn mouse_frame(buttons: u8, x: u16, y: u16, wheel: i8) -> [u8; 7] {
    let [xlo, xhi] = x.to_le_bytes();
    let [ylo, yhi] = y.to_le_bytes();
    [0x02, buttons, xlo, xhi, ylo, yhi, wheel as u8]
}

/// The 1-byte keepalive the client sends periodically.
#[allow(dead_code)] // documented protocol constant; sender not implemented yet
pub const HEARTBEAT: [u8; 1] = [0x00];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_press_and_release_match_capture() {
        // Observed: Esc (HID usage 0x29) down then up.
        let mut kb = KeyboardState::default();
        kb.press(0x29);
        assert_eq!(kb.report(), [0x01, 0, 0, 0x29, 0, 0, 0, 0, 0]);
        kb.release(0x29);
        assert_eq!(kb.report(), [0x01, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn modifier_sets_bit_not_keyslot() {
        let mut kb = KeyboardState::default();
        kb.press(0xE1); // Left Shift -> bit 1
        assert_eq!(kb.report(), [0x01, 0b0000_0010, 0, 0, 0, 0, 0, 0, 0]);
        kb.press(0x04); // 'a' while shift held
        assert_eq!(kb.report(), [0x01, 0b0000_0010, 0, 0x04, 0, 0, 0, 0, 0]);
        kb.release(0xE1);
        assert_eq!(kb.report(), [0x01, 0, 0, 0x04, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn mouse_center_and_button() {
        // Center ~ 16384 = 0x4000 -> little-endian 0x00,0x40.
        assert_eq!(
            mouse_frame(0, 16384, 16384, 0),
            [0x02, 0, 0x00, 0x40, 0x00, 0x40, 0]
        );
        // Left button pressed (bit0), observed byte1 = 0x01.
        assert_eq!(mouse_frame(0x01, 0, 0, 0)[1], 0x01);
    }
}
