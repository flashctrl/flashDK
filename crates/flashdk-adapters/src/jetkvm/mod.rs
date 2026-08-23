//! JetKVM adapter — control rides WebRTC DataChannels ([`TransportKind::PeerRpc`]).
//!
//! A WebRTC peer connection (signaling at `/webrtc/session`) must be negotiated before
//! *any* control. JetKVM splits functions across channels: a `rpc` channel carries
//! JSON-RPC 2.0 for control (video/EDID/power/etc.), while HID rides separate **binary**
//! `hidrpc*` channels. The HID frame formats are decoded in the `wire` module and the
//! sans-IO WebRTC transport lives in the `transport` module — both from wire captures
//! (docs/captures/jetkvm-datachannel-hid.md), never from device source.
//!
//! HID is live: [`JetKvm::connect`] logs in, brings up the peer connection, and
//! keyboard/mouse are sent as `wire` frames over the data channels. Power and virtual
//! media (which use the `rpc` JSON-RPC channel) are not wired yet.

mod transport;
mod wire;

use std::sync::Mutex;

use flashdk_core::capability::Vendor;
use flashdk_core::hid::{AbsMouse, Hid, KeyEvent, RelMouse, Wheel};
use flashdk_core::media::{MediaImage, VirtualMedia};
use flashdk_core::power::{Power, PowerAction, PowerState};
use flashdk_core::{Capabilities, Device, DeviceInfo, Error, Result, TransportKind};

use transport::JetTransport;

/// A connected JetKVM.
pub struct JetKvm {
    host: String,
    transport: JetTransport,
    /// Last absolute cursor position, so a wheel-only event can re-send it (JetKVM's
    /// mouse frame is absolute and always carries a position).
    mouse: Mutex<(u16, u16)>,
}

impl JetKvm {
    /// Log in and establish the WebRTC connection to `host` (e.g. "10.0.10.21").
    pub async fn connect(host: impl Into<String>, password: &str) -> Result<Self> {
        let host = host.into();
        let http = reqwest::Client::builder()
            .cookie_store(true) // carry the authToken cookie into signaling
            .build()
            .map_err(|e| Error::Transport(e.to_string()))?;

        let body = serde_json::json!({ "password": password }).to_string();
        let resp = http
            .post(format!("http://{host}/auth/login-local"))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Auth(format!("login failed: HTTP {}", resp.status())));
        }

        let transport = transport::connect(http, &host).await?;
        Ok(Self {
            host,
            transport,
            mouse: Mutex::new((0, 0)),
        })
    }

    /// The device host, for reference.
    pub fn host(&self) -> &str {
        &self.host
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
            relative_mouse: false, // only the absolute frame (type 0x03) is decoded
            video_mjpeg: false,    // WebRTC only — no MJPEG fallback
            video_h264: true,
            video_webrtc: true,
            power_on_off: true, // via ATX/DC extension hardware
            power_reset: true,
            virtual_media: true,
            wake_on_lan: true,   // getWakeOnLanDevices (rpc channel)
            tls_pinnable: false, // HTTP signaling; media secured by WebRTC's own DTLS
        }
    }

    fn transport_kind(&self) -> TransportKind {
        TransportKind::PeerRpc
    }
}

impl Hid for JetKvm {
    async fn key(&self, event: KeyEvent) -> Result<()> {
        // JetKVM keyboard frames carry raw USB HID usage codes — direct from core.
        self.transport
            .send_hid(wire::key_event(event.key.0, event.pressed).to_vec())
    }

    async fn absolute_mouse(&self, m: AbsMouse) -> Result<()> {
        {
            let mut pos = self
                .mouse
                .lock()
                .map_err(|_| Error::Protocol("mouse state poisoned".into()))?;
            *pos = (m.x, m.y);
        }
        self.transport
            .send_hid_unreliable(wire::mouse_abs(m.buttons, m.x, m.y, 0).to_vec())
    }

    async fn relative_mouse(&self, _m: RelMouse) -> Result<()> {
        Err(Error::NotSupported("relative mouse"))
    }

    async fn wheel(&self, w: Wheel) -> Result<()> {
        let (x, y) = {
            let pos = self
                .mouse
                .lock()
                .map_err(|_| Error::Protocol("mouse state poisoned".into()))?;
            *pos
        };
        self.transport
            .send_hid_unreliable(wire::mouse_abs(0, x, y, w.delta).to_vec())
    }
}

// Power and virtual media use the JSON-RPC `rpc` channel — not wired yet.
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
