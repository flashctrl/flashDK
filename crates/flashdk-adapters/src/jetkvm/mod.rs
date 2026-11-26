//! JetKVM adapter: control rides WebRTC DataChannels ([`TransportKind::PeerRpc`]).
//!
//! A WebRTC peer connection (signaling at `/webrtc/session`) must be negotiated before
//! *any* control. JetKVM splits functions across channels: a `rpc` channel carries
//! JSON-RPC 2.0 for control (video/EDID/power/etc.), while HID rides separate **binary**
//! `hidrpc*` channels. The HID frame formats are decoded in the `wire` module and the
//! sans-IO WebRTC transport lives in the `transport` module, both from wire captures
//! (docs/captures/jetkvm-datachannel-hid.md), never from device source.
//!
//! HID is live: [`JetKvm::connect`] logs in, brings up the peer connection, and
//! keyboard/mouse are sent as `wire` frames over the data channels. Virtual media rides
//! the `rpc` JSON-RPC channel (docs/captures/jetkvm-rpc-virtual-media.md). Power
//! (ATX/DC extensions) is not wired yet.

mod transport;
mod wire;

use std::sync::{Arc, Mutex};

use flashdk_core::capability::Vendor;
use flashdk_core::hid::{AbsMouse, Hid, KeyEvent, RelMouse, Wheel};
use flashdk_core::media::{MediaImage, VirtualMedia};
use flashdk_core::power::{Power, PowerAction, PowerState};
use flashdk_core::{
    Capabilities, ConnectionSecurity, Device, DeviceInfo, Error, Result, TlsPolicy, TransportKind,
};

use crate::scheme::Scheme;
use crate::tls_pin::{self, PinStore};
use crate::wsconn::{connect_with_policy, Attempt};
use transport::JetTransport;

/// A connected JetKVM.
pub struct JetKvm {
    scheme: Scheme,
    pins: Arc<dyn PinStore>,
    host: String,
    /// App version reported by the device at connect (via getLocalVersion).
    firmware: String,
    transport: JetTransport,
    /// Last absolute cursor position, so a wheel-only event can re-send it (JetKVM's
    /// mouse frame is absolute and always carries a position).
    mouse: Mutex<(u16, u16)>,
}

impl JetKvm {
    /// Log in and establish the WebRTC connection to `host` (e.g. "10.0.10.21").
    ///
    /// `policy` picks HTTPS or cleartext (see [`TlsPolicy`]); the login and the
    /// signaling WebSocket use the same scheme. WebRTC media is end-to-end DTLS either
    /// way, but its fingerprint travels in the signaling, so cleartext signaling
    /// leaves the session open to interception on the local network.
    pub async fn connect(
        host: impl Into<String>,
        password: &str,
        policy: TlsPolicy,
        store: Arc<dyn PinStore>,
    ) -> Result<Self> {
        let host = host.into();

        let ((cookie, scheme), _) = connect_with_policy(policy, &host, &store, |scheme| {
            let host = host.clone();
            let store = store.clone();
            let body = serde_json::json!({ "password": password }).to_string();
            async move {
                let (http, url) = match scheme {
                    Scheme::Https => (
                        tls_pin::tofu_client(&host, store),
                        format!("https://{host}/auth/login-local"),
                    ),
                    Scheme::Http => (
                        tls_pin::plain_client(),
                        format!("http://{host}/auth/login-local"),
                    ),
                };
                let http = http.map_err(|e| Attempt::fatal(Error::Transport(e)))?;
                let resp = http
                    .post(url)
                    .header("Content-Type", "application/json")
                    .body(body)
                    .send()
                    .await
                    .map_err(|e| Attempt::from_source(Error::Transport(e.to_string()), &e))?;
                if resp.status().is_redirection() {
                    return Err(crate::wsconn::redirect_error(resp.headers()));
                }
                if !resp.status().is_success() {
                    return Err(Attempt::fatal(Error::Auth(format!(
                        "login failed: HTTP {}",
                        resp.status()
                    ))));
                }
                let cookie = resp
                    .headers()
                    .get_all(reqwest::header::SET_COOKIE)
                    .iter()
                    .filter_map(|v| v.to_str().ok())
                    .filter_map(|v| v.split(';').next())
                    .collect::<Vec<_>>()
                    .join("; ");
                Ok((cookie, scheme))
            }
        })
        .await?;

        let transport = transport::connect(&cookie, &host, scheme, &store).await?;

        // Best-effort: read the app version over the rpc channel for DeviceInfo.
        let firmware = transport
            .rpc_call("getLocalVersion", serde_json::json!({}))
            .await
            .ok()
            .and_then(|v| v["appVersion"].as_str().map(str::to_string))
            .unwrap_or_else(|| "unknown".to_string());

        Ok(Self {
            scheme,
            pins: store,
            host,
            firmware,
            transport,
            mouse: Mutex::new((0, 0)),
        })
    }

    /// The device host, for reference.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Fetch the device's version info over the JSON-RPC `rpc` channel
    /// (`getLocalVersion`), a read-only control call, useful as a liveness check and
    /// to populate firmware info later.
    pub async fn local_version(&self) -> Result<serde_json::Value> {
        self.transport
            .rpc_call("getLocalVersion", serde_json::json!({}))
            .await
    }
}

impl Device for JetKvm {
    fn info(&self) -> DeviceInfo {
        DeviceInfo {
            vendor: Vendor::JetKvm,
            model: "JetKVM".to_string(),
            firmware: self.firmware.clone(),
            hardened: false,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            keyboard: true,
            absolute_mouse: true,
            relative_mouse: false, // only the absolute frame (type 0x03) is decoded
            video_mjpeg: false,    // WebRTC only, no MJPEG fallback
            video_h264: true,
            video_webrtc: true,
            power_on_off: true, // via ATX/DC extension hardware
            power_reset: true,
            virtual_media: true,
            wake_on_lan: true,  // getWakeOnLanDevices (rpc channel)
            tls_pinnable: true, // when HTTPS is enabled on the device
        }
    }

    fn security(&self) -> ConnectionSecurity {
        match self.scheme {
            Scheme::Https => ConnectionSecurity::Tls {
                pinned: self.pins.get(&self.host),
            },
            Scheme::Http => ConnectionSecurity::Cleartext,
        }
    }

    fn transport_kind(&self) -> TransportKind {
        TransportKind::PeerRpc
    }
}

impl Hid for JetKvm {
    async fn key(&self, event: KeyEvent) -> Result<()> {
        // JetKVM keyboard frames carry raw USB HID usage codes, direct from core.
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

// Power rides the JSON-RPC `rpc` channel via the ATX/DC extensions; not captured yet.
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
        let files = self
            .transport
            .rpc_call("listStorageFiles", serde_json::json!({}))
            .await?;
        let state = self
            .transport
            .rpc_call("getVirtualMediaState", serde_json::json!({}))
            .await?;
        let mounted = wire::parse_mounted_filename(&state);
        Ok(wire::parse_storage_files(&files)
            .into_iter()
            .map(|f| MediaImage {
                mounted: mounted.as_deref() == Some(f.filename.as_str()),
                name: f.filename,
                size: Some(f.size),
            })
            .collect())
    }

    /// Mounts a stored image as a CD/DVD (the only mode captured so far).
    async fn mount(&self, name: &str) -> Result<()> {
        self.transport
            .rpc_call("mountWithStorage", wire::mount_with_storage_params(name))
            .await
            .map(|_| ())
    }

    async fn unmount(&self) -> Result<()> {
        self.transport
            .rpc_call("unmountImage", serde_json::json!({}))
            .await
            .map(|_| ())
    }
}
