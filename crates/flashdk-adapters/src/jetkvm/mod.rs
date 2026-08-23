//! JetKVM adapter — control rides WebRTC DataChannels ([`TransportKind::PeerRpc`]).
//!
//! The odd one out: a WebRTC peer connection (signaling at `/webrtc/session`) must be
//! negotiated before *any* control. JetKVM then splits functions across channels: a
//! `rpc` channel carries JSON-RPC 2.0 for control (video/EDID/power/etc.), while HID
//! rides separate **binary** `hidrpc*` channels. The HID frame formats are decoded in
//! the `wire` module from DataChannel captures (see
//! docs/captures/jetkvm-datachannel-hid.md), never from device source.
//!
//! The WebRTC transport itself (webrtc-rs) is not wired yet; this adapter remains a
//! stub until it lands. The wire encoders below are complete and unit-tested.

mod transport;
mod wire;

use flashdk_core::capability::Vendor;
use flashdk_core::hid::{AbsMouse, Hid, KeyEvent, RelMouse, Wheel};
use flashdk_core::media::{MediaImage, VirtualMedia};
use flashdk_core::power::{Power, PowerAction, PowerState};
use flashdk_core::{Capabilities, Device, DeviceInfo, Error, Result, TransportKind};

pub struct JetKvm {
    pub host: String,
}

impl JetKvm {
    pub fn new(host: impl Into<String>) -> Self {
        Self { host: host.into() }
    }
}

impl Device for JetKvm {
    fn info(&self) -> DeviceInfo {
        DeviceInfo {
            vendor: Vendor::JetKvm,
            model: "JetKVM".to_string(),
            firmware: "unknown".to_string(),
            hardened: false,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            keyboard: true,
            absolute_mouse: true,
            relative_mouse: true,
            video_mjpeg: false, // WebRTC only — no MJPEG fallback
            video_h264: true,
            video_webrtc: true,
            power_on_off: true, // via ATX/DC extension hardware
            power_reset: true,
            virtual_media: true,
            wake_on_lan: true,   // getWakeOnLanDevices
            tls_pinnable: false, // HTTP signaling; media secured by WebRTC's own DTLS
        }
    }

    fn transport_kind(&self) -> TransportKind {
        TransportKind::PeerRpc
    }
}

impl Hid for JetKvm {
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

impl Power for JetKvm {
    async fn action(&self, _action: PowerAction) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn state(&self) -> Result<PowerState> {
        Err(Error::NotImplemented)
    }
}

impl VirtualMedia for JetKvm {
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
