//! NanoKVM PCIe adapter — REST + WebSocket, cleartext HTTP.
//!
//! Capabilities and transport below reflect what we observed live on app 2.5.0
//! (10.0.10.10). See `PROVENANCE.md` in this folder. All actions are stubs today.

use flashdk_core::capability::Vendor;
use flashdk_core::hid::{AbsMouse, Hid, KeyEvent, RelMouse, Wheel};
use flashdk_core::media::{MediaImage, VirtualMedia};
use flashdk_core::power::{Power, PowerAction, PowerState};
use flashdk_core::{Capabilities, Device, DeviceInfo, Error, Result, TransportKind};

/// A connected NanoKVM. Real fields (base URL, JWT cookie, HTTP client) arrive when we
/// wire behaviour; for now it just remembers where the device is.
pub struct NanoKvm {
    pub host: String,
}

impl NanoKvm {
    /// Construct an adapter pointed at `host` (e.g. "10.0.10.10"). Does not connect yet.
    pub fn new(host: impl Into<String>) -> Self {
        Self { host: host.into() }
    }
}

impl Device for NanoKvm {
    fn info(&self) -> DeviceInfo {
        DeviceInfo {
            vendor: Vendor::NanoKvm,
            model: "NanoKVM PCIe".to_string(),
            firmware: "unknown".to_string(),
            hardened: false,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            keyboard: true,
            absolute_mouse: true,
            relative_mouse: true,
            video_mjpeg: true,
            video_h264: true,
            video_webrtc: false, // STUN config present; unverified — stay conservative
            power_on_off: true,
            power_reset: false, // GPIO pulse only, no true reset line
            virtual_media: true,
            wake_on_lan: false,
            tls_pinnable: false, // ships cleartext HTTP — nothing to pin
        }
    }

    fn transport_kind(&self) -> TransportKind {
        TransportKind::RequestResponse
    }
}

impl Hid for NanoKvm {
    async fn key(&self, _event: KeyEvent) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn absolute_mouse(&self, _m: AbsMouse) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn relative_mouse(&self, _m: RelMouse) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn wheel(&self, _w: Wheel) -> Result<()> {
        Err(Error::NotImplemented)
    }
}

impl Power for NanoKvm {
    async fn action(&self, _action: PowerAction) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn state(&self) -> Result<PowerState> {
        Err(Error::NotImplemented)
    }
}

impl VirtualMedia for NanoKvm {
    async fn list(&self) -> Result<Vec<MediaImage>> {
        Err(Error::NotImplemented)
    }
    async fn mount(&self, _name: &str) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn unmount(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
}
