//! What ties the capability traits together into "a device you can drive".
//!
//! An adapter implements the input/power/media traits *and* [`Device`], which adds the
//! self-description the app needs: who it is, what it can do, and which transport shape
//! it uses. Note there is **no vendor-specific type in this crate** — the concrete
//! `Kvm` dispatcher lives in `flashdk-adapters`, keeping core vendor-agnostic.

use crate::capability::{Capabilities, Vendor};
use crate::hid::Hid;
use crate::media::VirtualMedia;
use crate::power::Power;
use crate::transport::TransportKind;

/// Identity and firmware details, filled in after connecting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub vendor: Vendor,
    /// Human-readable model, e.g. "NanoKVM PCIe" or "PiKVM v3".
    pub model: String,
    /// Firmware/app version string as the device reports it.
    pub firmware: String,
    /// True once we detect flashCtrl's own hardened firmware (the "Hardened ✓" badge).
    /// Always false for stock firmware.
    pub hardened: bool,
}

/// The umbrella contract: anything that is a drivable KVM implements the three
/// capability traits **and** can describe itself.
///
/// Because the capability traits use `async fn`, `Device` is not object-safe — you
/// can't write `Box<dyn Device>`. That's deliberate: for runtime polymorphism across
/// vendors we use the `Kvm` enum in `flashdk-adapters`, which is faster and clearer
/// than dynamic dispatch anyway.
pub trait Device: Hid + Power + VirtualMedia {
    /// Static identity/firmware info (cheap; no network call).
    fn info(&self) -> DeviceInfo;

    /// What this specific device supports. The app reads this to decide what to show.
    fn capabilities(&self) -> Capabilities;

    /// Which transport shape this device uses — the app checks
    /// [`TransportKind::requires_peer_connection`] to know whether to bring up WebRTC
    /// before enabling controls.
    fn transport_kind(&self) -> TransportKind;
}
