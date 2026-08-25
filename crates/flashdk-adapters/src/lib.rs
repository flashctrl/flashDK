//! # flashdk-adapters
//!
//! One module per vendor, each teaching `flashdk-core`'s traits how to speak to a
//! real device. Every line here is derived from **wire observation and official docs
//! only**, never from any vendor's source. See `CLEANROOM.md`.
//!
//! Status: PiKVM and NanoKVM have live HID + power + virtual media; JetKVM has live HID
//! over WebRTC (power/media over its JSON-RPC channel are pending). The [`Kvm`] enum
//! below is the vendor-neutral entry point apps hold.

#![allow(async_fn_in_trait)]

pub mod jetkvm;
pub mod nanokvm;
pub mod pikvm;
pub mod tls_pin;

use flashdk_core::hid::{AbsMouse, Hid, KeyEvent, RelMouse, Wheel};
use flashdk_core::media::{MediaImage, VirtualMedia};
use flashdk_core::power::{Power, PowerAction, PowerState};
use flashdk_core::{Capabilities, Device, DeviceInfo, Result, TransportKind};

/// Runtime polymorphism across vendors without `dyn`. The app holds a `Kvm` and calls
/// methods on it; each call fans out to the concrete adapter inside. Adding a vendor
/// means adding a variant and four `match` arms; the compiler makes sure you don't
/// forget one.
pub enum Kvm {
    NanoKvm(nanokvm::NanoKvm),
    PiKvm(pikvm::PiKvm),
    JetKvm(jetkvm::JetKvm),
}

impl Kvm {
    /// Static identity, no network call.
    pub fn info(&self) -> DeviceInfo {
        match self {
            Kvm::NanoKvm(a) => a.info(),
            Kvm::PiKvm(a) => a.info(),
            Kvm::JetKvm(a) => a.info(),
        }
    }

    /// What this device supports.
    pub fn capabilities(&self) -> Capabilities {
        match self {
            Kvm::NanoKvm(a) => a.capabilities(),
            Kvm::PiKvm(a) => a.capabilities(),
            Kvm::JetKvm(a) => a.capabilities(),
        }
    }

    /// Transport shape: check before enabling controls (JetKVM needs WebRTC first).
    pub fn transport_kind(&self) -> TransportKind {
        match self {
            Kvm::NanoKvm(a) => a.transport_kind(),
            Kvm::PiKvm(a) => a.transport_kind(),
            Kvm::JetKvm(a) => a.transport_kind(),
        }
    }

    // --- HID: the first vertical slice. Forwarded so app code calls `kvm.key(..)`. ---

    pub async fn key(&self, event: KeyEvent) -> Result<()> {
        match self {
            Kvm::NanoKvm(a) => a.key(event).await,
            Kvm::PiKvm(a) => a.key(event).await,
            Kvm::JetKvm(a) => a.key(event).await,
        }
    }

    pub async fn absolute_mouse(&self, m: AbsMouse) -> Result<()> {
        match self {
            Kvm::NanoKvm(a) => a.absolute_mouse(m).await,
            Kvm::PiKvm(a) => a.absolute_mouse(m).await,
            Kvm::JetKvm(a) => a.absolute_mouse(m).await,
        }
    }

    pub async fn relative_mouse(&self, m: RelMouse) -> Result<()> {
        match self {
            Kvm::NanoKvm(a) => a.relative_mouse(m).await,
            Kvm::PiKvm(a) => a.relative_mouse(m).await,
            Kvm::JetKvm(a) => a.relative_mouse(m).await,
        }
    }

    pub async fn wheel(&self, w: Wheel) -> Result<()> {
        match self {
            Kvm::NanoKvm(a) => a.wheel(w).await,
            Kvm::PiKvm(a) => a.wheel(w).await,
            Kvm::JetKvm(a) => a.wheel(w).await,
        }
    }

    // --- Power & media: forwarded too, so the surface is complete. ---

    pub async fn power_action(&self, action: PowerAction) -> Result<()> {
        match self {
            Kvm::NanoKvm(a) => a.action(action).await,
            Kvm::PiKvm(a) => a.action(action).await,
            Kvm::JetKvm(a) => a.action(action).await,
        }
    }

    pub async fn power_state(&self) -> Result<PowerState> {
        match self {
            Kvm::NanoKvm(a) => a.state().await,
            Kvm::PiKvm(a) => a.state().await,
            Kvm::JetKvm(a) => a.state().await,
        }
    }

    pub async fn media_list(&self) -> Result<Vec<MediaImage>> {
        match self {
            Kvm::NanoKvm(a) => a.list().await,
            Kvm::PiKvm(a) => a.list().await,
            Kvm::JetKvm(a) => a.list().await,
        }
    }

    /// Mount a virtual-media image by name.
    pub async fn media_mount(&self, name: &str) -> Result<()> {
        match self {
            Kvm::NanoKvm(a) => a.mount(name).await,
            Kvm::PiKvm(a) => a.mount(name).await,
            Kvm::JetKvm(a) => a.mount(name).await,
        }
    }

    /// Unmount whatever virtual media is currently presented.
    pub async fn media_unmount(&self) -> Result<()> {
        match self {
            Kvm::NanoKvm(a) => a.unmount().await,
            Kvm::PiKvm(a) => a.unmount().await,
            Kvm::JetKvm(a) => a.unmount().await,
        }
    }

    /// Type a string as key events (US layout; see [`flashdk_core::hid::Hid::paste_text`]).
    pub async fn paste_text(&self, text: &str) -> Result<()> {
        match self {
            Kvm::NanoKvm(a) => a.paste_text(text).await,
            Kvm::PiKvm(a) => a.paste_text(text).await,
            Kvm::JetKvm(a) => a.paste_text(text).await,
        }
    }
}
