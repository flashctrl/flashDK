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

    /// Convenience: type a whole string. The default implementation is intentionally
    /// left for adapters/helpers to fill — turning text into keycodes is layout-
    /// dependent and worth doing once, carefully, in shared code later.
    async fn paste_text(&self, _text: &str) -> Result<()> {
        Err(crate::error::Error::NotImplemented)
    }
}
