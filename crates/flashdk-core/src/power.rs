//! Machine power control: one concept, three mechanisms underneath.
//!
//! Not every action exists on every device (NanoKVM has no true reset). Adapters
//! return [`Error::NotSupported`](crate::Error::NotSupported) for actions their
//! hardware can't do, and the app
//! should consult [`Capabilities`](crate::Capabilities) before offering the button.

use crate::error::Result;

/// A power action to request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    /// Turn on (or short-press the power button on a machine that's off).
    On,
    /// Graceful short press: asks the OS to shut down.
    ShortPress,
    /// Forceful long press: cuts power after a held button.
    LongPress,
    /// Hardware reset line. Not all devices have this.
    Reset,
}

/// What the device reports about the attached machine's power/activity LEDs.
/// `Option` because some devices can't sense a given signal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PowerState {
    pub powered: Option<bool>,
    pub hdd_activity: Option<bool>,
}

/// The power contract adapters implement.
pub trait Power {
    async fn action(&self, action: PowerAction) -> Result<()>;
    async fn state(&self) -> Result<PowerState>;
}
