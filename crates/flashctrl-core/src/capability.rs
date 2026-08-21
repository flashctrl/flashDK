//! What a given device can do — discovered, never assumed.
//!
//! The golden rule from our probing: **capability-flag everything, assume nothing.**
//! Power is the classic trap — it's one concept (turn the machine on/off) over three
//! totally different mechanisms, and NanoKVM can't do a true ATX *reset* at all. So
//! the app must ask [`Capabilities`] what's present and render accordingly.

/// Which vendor an adapter speaks for. Used for labelling and for the small number
/// of places behaviour legitimately branches on brand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    NanoKvm,
    PiKvm,
    JetKvm,
    /// GL.iNet Comet — untested; we don't own the hardware yet. Present so the type
    /// is complete, but no adapter ships until we've probed a real unit.
    GlInet,
}

/// A snapshot of what one device supports, filled in by its adapter after connecting.
/// Booleans (not an enum set) keep this trivially readable from Swift/Kotlin once the
/// UniFFI bindings exist.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Capabilities {
    // --- Input (unifies cleanly across every device) ---
    pub keyboard: bool,
    pub absolute_mouse: bool,
    pub relative_mouse: bool,

    // --- Video (the hard layer: three different transport shapes) ---
    pub video_mjpeg: bool,
    pub video_h264: bool,
    pub video_webrtc: bool,

    // --- Power (common concept, split mechanism) ---
    pub power_on_off: bool,
    /// True ATX reset line. PiKVM: yes. NanoKVM: no (GPIO pulse only).
    pub power_reset: bool,

    // --- Storage & extras ---
    pub virtual_media: bool,
    pub wake_on_lan: bool,

    // --- Security posture the app must surface honestly ---
    /// Device offers TLS the client can pin. PiKVM: yes. NanoKVM: no (cleartext).
    pub tls_pinnable: bool,
}
