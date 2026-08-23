//! Human Interface Device: keyboard and mouse — the layer that unifies most cleanly.
//!
//! Every target device ultimately accepts the same thing: USB HID reports. The
//! envelopes differ (PiKVM/NanoKVM take REST or WebSocket messages; JetKVM takes a
//! `keyboardReport` JSON-RPC call over its DataChannel) but the *meaning* is identical.
//! So this is where we build first — one interface, thin per-vendor encoders.
//!
//! Key codes here are **USB HID usage IDs**, a public standard (USB-IF HID Usage
//! Tables). Using the standard — rather than any vendor's key map — keeps us squarely
//! clean-room: we encode to a spec everyone shares, not to anyone's source.

use crate::error::Result;

/// A USB HID keyboard usage ID (e.g. 0x04 = 'a', 0x28 = Enter). Newtype around a
/// `u8` so we can't accidentally mix it up with other numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCode(pub u8);

/// Left Shift modifier usage id — used to type shifted characters.
pub const LEFT_SHIFT: KeyCode = KeyCode(0xE1);

/// Map a character to its US-layout HID usage id and whether Shift is required.
///
/// Uses the USB HID Usage Tables (page 0x07) and the US ANSI keyboard layout — both
/// public standards, so this stays clean-room. Returns `None` for characters not
/// reachable on a US layout (callers typing text should skip those).
pub fn char_to_hid(c: char) -> Option<(KeyCode, bool)> {
    let (usage, shift): (u8, bool) = match c {
        'a'..='z' => (0x04 + (c as u8 - b'a'), false),
        'A'..='Z' => (0x04 + (c as u8 - b'A'), true),
        '1'..='9' => (0x1E + (c as u8 - b'1'), false),
        '0' => (0x27, false),
        // Shifted number row (US).
        '!' => (0x1E, true),
        '@' => (0x1F, true),
        '#' => (0x20, true),
        '$' => (0x21, true),
        '%' => (0x22, true),
        '^' => (0x23, true),
        '&' => (0x24, true),
        '*' => (0x25, true),
        '(' => (0x26, true),
        ')' => (0x27, true),
        // Whitespace / control.
        ' ' => (0x2C, false),
        '\n' => (0x28, false), // Enter
        '\t' => (0x2B, false), // Tab
        // Punctuation and their shifted pairs.
        '-' => (0x2D, false),
        '_' => (0x2D, true),
        '=' => (0x2E, false),
        '+' => (0x2E, true),
        '[' => (0x2F, false),
        '{' => (0x2F, true),
        ']' => (0x30, false),
        '}' => (0x30, true),
        '\\' => (0x31, false),
        '|' => (0x31, true),
        ';' => (0x33, false),
        ':' => (0x33, true),
        '\'' => (0x34, false),
        '"' => (0x34, true),
        '`' => (0x35, false),
        '~' => (0x35, true),
        ',' => (0x36, false),
        '<' => (0x36, true),
        '.' => (0x37, false),
        '>' => (0x37, true),
        '/' => (0x38, false),
        '?' => (0x38, true),
        _ => return None,
    };
    Some((KeyCode(usage), shift))
}

/// One key going down or coming up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: KeyCode,
    /// true = pressed, false = released.
    pub pressed: bool,
}

/// The three mouse buttons we model. (Extra buttons can come later behind a capability.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Absolute pointer position, normalised to 0..=32767 on each axis (the USB absolute
/// mouse convention). Absolute positioning is what makes a remote cursor track your
/// finger/mouse 1:1 without drift — preferred wherever the device supports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbsMouse {
    pub x: u16,
    pub y: u16,
    /// Bitmask of currently-held buttons (bit 0 = left, 1 = right, 2 = middle).
    pub buttons: u8,
}

/// Relative pointer movement (deltas). The fallback when absolute isn't available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelMouse {
    pub dx: i16,
    pub dy: i16,
    pub buttons: u8,
}

/// Scroll wheel tick. Positive = up/away from the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wheel {
    pub delta: i8,
}

/// The keyboard/mouse contract every adapter implements.
///
/// These are `async` because they cross the network. An adapter turns each call into
/// whatever its device expects — an HTTP POST, a WebSocket frame, or a DataChannel
/// RPC — while callers stay blissfully unaware of which.
pub trait Hid {
    /// Send a single key press or release.
    async fn key(&self, event: KeyEvent) -> Result<()>;

    /// Move/click using absolute coordinates. Errors with `NotSupported` if the device
    /// only does relative movement.
    async fn absolute_mouse(&self, m: AbsMouse) -> Result<()>;

    /// Move/click using relative deltas.
    async fn relative_mouse(&self, m: RelMouse) -> Result<()>;

    /// Scroll.
    async fn wheel(&self, w: Wheel) -> Result<()>;

    /// Convenience: type a whole string as individual key events (US layout).
    ///
    /// The default drives [`key`](Hid::key) per character via [`char_to_hid`], so every
    /// adapter gets working paste for free. Characters not reachable on a US layout are
    /// skipped (best-effort). Adapters with a native bulk-paste endpoint may override
    /// this for efficiency.
    async fn paste_text(&self, text: &str) -> Result<()> {
        for ch in text.chars() {
            let Some((code, shift)) = char_to_hid(ch) else {
                continue; // skip characters we can't type on a US layout
            };
            if shift {
                self.key(KeyEvent {
                    key: LEFT_SHIFT,
                    pressed: true,
                })
                .await?;
            }
            self.key(KeyEvent {
                key: code,
                pressed: true,
            })
            .await?;
            self.key(KeyEvent {
                key: code,
                pressed: false,
            })
            .await?;
            if shift {
                self.key(KeyEvent {
                    key: LEFT_SHIFT,
                    pressed: false,
                })
                .await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_layout_mapping() {
        assert_eq!(char_to_hid('a'), Some((KeyCode(0x04), false)));
        assert_eq!(char_to_hid('A'), Some((KeyCode(0x04), true)));
        assert_eq!(char_to_hid('z'), Some((KeyCode(0x1D), false)));
        assert_eq!(char_to_hid('1'), Some((KeyCode(0x1E), false)));
        assert_eq!(char_to_hid('0'), Some((KeyCode(0x27), false)));
        assert_eq!(char_to_hid('!'), Some((KeyCode(0x1E), true)));
        assert_eq!(char_to_hid(')'), Some((KeyCode(0x27), true)));
        assert_eq!(char_to_hid(' '), Some((KeyCode(0x2C), false)));
        assert_eq!(char_to_hid('\n'), Some((KeyCode(0x28), false)));
        assert_eq!(char_to_hid('?'), Some((KeyCode(0x38), true)));
        assert_eq!(char_to_hid('_'), Some((KeyCode(0x2D), true)));
        assert_eq!(char_to_hid('€'), None); // not on US layout
    }
}
