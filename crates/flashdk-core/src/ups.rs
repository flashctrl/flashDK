//! UPS telemetry and the small set of commands a UPS actually supports.
//!
//! Like [`outlet`](crate::outlet), this is standalone rather than part of the
//! [`Device`](crate::Device) umbrella: a UPS has no HID, no video, and (for a
//! consumer unit like an APC Back-UPS) no switched outlets either. It's
//! monitoring-first by nature. A networked, outlet-switching UPS (an APC Smart-UPS
//! with a Network Management Card, say) would additionally implement
//! [`PowerOutlet`](crate::outlet::PowerOutlet); a USB-only consumer unit reached
//! through NUT or apcupsd implements only this trait.

use crate::error::Result;

/// Whether the UPS is drawing from line power or its battery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerSource {
    /// Running on utility power; the battery is charging or topped off.
    Line,
    /// Utility power is out; running on battery.
    Battery,
}

/// A snapshot of UPS telemetry. Every field is `Option` because not every UPS, or
/// every backend (NUT vs. apcupsd vs. SNMP), reports everything.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UpsState {
    pub source: Option<PowerSource>,
    /// Battery charge, 0.0 to 100.0.
    pub charge_percent: Option<f32>,
    /// Load on the UPS as a percentage of its rated capacity.
    pub load_percent: Option<f32>,
    /// Estimated remaining runtime on battery, in seconds, if the UPS is discharging.
    pub runtime_seconds: Option<u32>,
}

/// The small set of commands a consumer UPS actually accepts. Deliberately not a
/// general command-passthrough: a Back-UPS has no switched outlets to act on, so
/// there is nothing here beyond diagnostics and the beeper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsCommand {
    /// Run the UPS's self-test and report to its own status, not this call directly.
    SelfTest,
    MuteBeeper,
    UnmuteBeeper,
}

/// The contract a UPS adapter implements.
pub trait UpsStatus {
    /// Read current telemetry.
    async fn state(&self) -> Result<UpsState>;
    /// Issue one of the UPS's supported commands. Returns
    /// [`Error::NotSupported`](crate::Error::NotSupported) for a command this
    /// specific unit doesn't have (a Back-UPS has no outlets to switch, for
    /// instance, so a switching command would land here if one were ever added).
    async fn command(&self, cmd: UpsCommand) -> Result<()>;
}
