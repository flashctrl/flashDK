//! Switched power outlets, as found on a networked PDU (e.g. a Ubiquiti UniFi PDU).
//!
//! This is deliberately not part of the [`Device`](crate::Device) umbrella trait.
//! `Device` requires [`Hid`](crate::hid::Hid), [`Power`](crate::power::Power), and
//! [`VirtualMedia`](crate::media::VirtualMedia), because every KVM has all three
//! concepts even when a given device can't act on one of them. A PDU has none of
//! those: it has no keyboard or mouse to inject, no single "the machine" to power
//! cycle (it has many, one per outlet), and nothing resembling removable media.
//! Forcing a PDU adapter to implement `Hid` just to satisfy a shared umbrella trait
//! would mean every method returning [`Error::NotSupported`](crate::Error), which
//! tells a caller nothing a capability check couldn't already tell them more
//! honestly. See docs/decisions.md for the fuller reasoning once a PDU adapter
//! actually lands.
//!
//! [`Power`](crate::power::Power) still covers the KVM case of power-cycling *the
//! one machine a KVM is attached to*. This trait covers a device that switches
//! *N independently addressable outlets*, which is a different shape of problem
//! even though both are "turn something's power on or off."

use crate::error::Result;

/// One outlet on a PDU, as the device numbers or names it.
// `watts` is a float, so this can only derive `PartialEq`, not `Eq`.
#[derive(Debug, Clone, PartialEq)]
pub struct Outlet {
    /// The device's own identifier for this outlet (an index or a user-assigned
    /// name), used as the argument to [`PowerOutlet`] methods.
    pub id: String,
    /// A human-readable label, if the device or its owner has set one.
    pub name: Option<String>,
    /// Whether the outlet is currently energized.
    pub on: bool,
    /// Live power draw in watts, on models that meter individual outlets.
    pub watts: Option<f32>,
}

/// The action to take on one outlet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutletAction {
    On,
    Off,
    /// Off, a brief pause, then on again: the PDU equivalent of unplugging and
    /// replugging a device that's stopped responding.
    Cycle,
}

/// The contract a switched-PDU adapter implements.
pub trait PowerOutlet {
    /// List every outlet the device knows about, with current state.
    async fn outlets(&self) -> Result<Vec<Outlet>>;
    /// Act on one outlet by its `id` (as returned from [`Self::outlets`]).
    async fn set_outlet(&self, id: &str, action: OutletAction) -> Result<()>;
}
