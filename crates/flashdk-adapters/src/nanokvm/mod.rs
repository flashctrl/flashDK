//! NanoKVM PCIe adapter: REST for auth/aux, and a binary WebSocket (`/api/ws`) for
//! live keyboard and mouse, derived from wire observation
//! (docs/captures/nanokvm-ws-hid.md).
//!
//! Password login is quarantined: it needed a secret that exists only in the vendor's
//! web client, which CLEANROOM.md forbids reading. See PROVENANCE.md in this directory.
//! A session token the user already holds can be supplied with
//! [`NanoKvm::connect_with_token`].

mod wire;

use flashdk_core::capability::Vendor;
use flashdk_core::hid::{AbsMouse, Hid, KeyEvent, RelMouse, Wheel};
use flashdk_core::media::{MediaImage, VirtualMedia};
use flashdk_core::power::{Power, PowerAction, PowerState};
use flashdk_core::{
    Capabilities, ConnectionSecurity, Device, DeviceInfo, Error, Result, TlsPolicy, TransportKind,
};

use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use tokio_tungstenite::tungstenite::Message;

use crate::scheme::Scheme;
use crate::tls_pin::{self, PinStore};
use crate::wsconn::{connect_with_policy, open_ws, Attempt, WsStream};
use wire::{mouse_frame, KeyboardState};

type WsSink = SplitSink<WsStream, Message>;

fn base_url(scheme: Scheme, host: &str) -> String {
    match scheme {
        Scheme::Https => format!("https://{host}"),
        Scheme::Http => format!("http://{host}"),
    }
}

/// Last-known absolute cursor position, so wheel/button-only events can re-send it.
#[derive(Default, Clone, Copy)]
struct MousePos {
    x: u16,
    y: u16,
}

/// A connected NanoKVM.
pub struct NanoKvm {
    host: String,
    base_url: String,
    scheme: Scheme,
    pins: Arc<dyn PinStore>,
    http: reqwest::Client,
    token: String,
    /// Firmware ("image") version read from /api/vm/info at connect, e.g. "v1.4.0".
    firmware: String,
    ws: AsyncMutex<WsSink>,
    keyboard: Mutex<KeyboardState>,
    mouse: Mutex<MousePos>,
}

/// NanoKVM's uniform reply shape: `{"code": 0, "msg": ..., "data": {...}}`.
#[derive(serde::Deserialize)]
struct NkResponse {
    code: i64,
    #[serde(default)]
    data: serde_json::Value,
}

/// `GET /api/storage/media/state` reports `"state":"unsupported"` while the virtual disk
/// is switched off in the device settings (observed live; with it on the state was
/// `"partition"` idle and `"image"` with an image mounted).
fn virtual_disk_disabled(data: &serde_json::Value) -> bool {
    data["state"].as_str() == Some("unsupported")
}

/// Pull the firmware/image version out of a `GET /api/vm/info` `data` payload
/// (`{"image":"v1.4.0","application":"2.5.0","deviceKey":"...",...}`, verified live
/// against a NanoKVM PCIe; see the unit tests below for the exact captured shape).
fn extract_image_version(data: &serde_json::Value) -> Option<String> {
    data["image"].as_str().map(str::to_string)
}

impl NanoKvm {
    /// Password login is **not available** and always returns `NotSupported`, before
    /// any network traffic.
    ///
    /// The device rejects a plain password, and the scheme it expects depends on a
    /// secret that appears only in the vendor's web client, never on the wire or in
    /// the vendor's documentation. We will not read that client, so we cannot
    /// implement it. This returns when Sipeed documents the login or permits it in
    /// writing. Until then, use [`Self::connect_with_token`].
    pub async fn connect(
        _host: impl Into<String>,
        _username: &str,
        _password: &str,
    ) -> Result<Self> {
        Err(Error::NotSupported(
            "NanoKVM password login (no clean-room source; use connect_with_token)",
        ))
    }

    /// Open the HID WebSocket with a session `token` the user already obtained by
    /// signing in through the device's own web interface (the `nano-kvm-token`
    /// cookie value). `host` is e.g. "192.0.2.10". Tokens expire, so callers must
    /// handle `Auth` errors by asking the user to sign in again.
    ///
    /// `policy` picks HTTPS or cleartext (see [`TlsPolicy`]). NanoKVM ships with HTTPS
    /// off, so a stock unit needs `PreferTls` or `CleartextOnly`; once HTTPS is
    /// enabled on the device, certificates are pinned in `store` on first contact.
    /// [`Device::security`] reports which one was actually used.
    pub async fn connect_with_token(
        host: impl Into<String>,
        token: &str,
        policy: TlsPolicy,
        store: Arc<dyn PinStore>,
    ) -> Result<Self> {
        let host = host.into();
        let token = token.to_string();

        let ((http, sink, firmware, scheme), _scheme) =
            connect_with_policy(policy, &host, &store, |scheme| {
                let host = host.clone();
                let token = token.clone();
                let store = store.clone();
                async move {
                    let cookie = format!("nano-kvm-token={token}");
                    let http = match scheme {
                        Scheme::Https => tls_pin::tofu_client(&host, store.clone()),
                        Scheme::Http => tls_pin::plain_client(),
                    }
                    .map_err(|e| Attempt::fatal(Error::Transport(e)))?;

                    let stream = open_ws(
                        scheme,
                        &host,
                        "/api/ws",
                        &[("Cookie", cookie.clone())],
                        &store,
                    )
                    .await?;
                    let (sink, _read) = stream.split();

                    // Best-effort: read the firmware/image version for DeviceInfo.
                    let base = base_url(scheme, &host);
                    let parsed = match http
                        .get(format!("{base}/api/vm/info"))
                        .header("Cookie", cookie)
                        .send()
                        .await
                    {
                        Ok(r) => r.json::<NkResponse>().await.ok(),
                        Err(_) => None,
                    };
                    let firmware = parsed
                        .and_then(|r| extract_image_version(&r.data))
                        .unwrap_or_else(|| "unknown".to_string());
                    Ok((http, sink, firmware, scheme))
                }
            })
            .await?;

        Ok(Self {
            base_url: base_url(scheme, &host),
            scheme,
            pins: store,
            host,
            http,
            token,
            firmware,
            ws: AsyncMutex::new(sink),
            keyboard: Mutex::new(KeyboardState::default()),
            mouse: Mutex::new(MousePos::default()),
        })
    }

    /// The device host, for reference.
    pub fn host(&self) -> &str {
        &self.host
    }

    async fn send(&self, bytes: Vec<u8>) -> Result<()> {
        let mut sink = self.ws.lock().await;
        sink.send(Message::Binary(bytes.into()))
            .await
            .map_err(|e| Error::Transport(e.to_string()))
    }

    /// GET a REST path, returning its `data` payload (auth via the token cookie).
    async fn get_data(&self, path: &str) -> Result<serde_json::Value> {
        let resp = self
            .http
            .get(format!("{}{}", self.base_url, path))
            .header("Cookie", format!("nano-kvm-token={}", self.token))
            .send()
            .await
            .map_err(|e| crate::tls_pin::transport_error(&e))?;
        let r: NkResponse = resp
            .json()
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;
        if r.code == 0 {
            Ok(r.data)
        } else {
            Err(Error::Protocol(format!("nanokvm code {}", r.code)))
        }
    }

    /// POST JSON to a REST path, returning its `data` payload.
    async fn post_json(&self, path: &str, body: serde_json::Value) -> Result<serde_json::Value> {
        let resp = self
            .http
            .post(format!("{}{}", self.base_url, path))
            .header("Cookie", format!("nano-kvm-token={}", self.token))
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::tls_pin::transport_error(&e))?;
        let r: NkResponse = resp
            .json()
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;
        if r.code == 0 {
            Ok(r.data)
        } else {
            Err(Error::Protocol(format!("nanokvm code {}", r.code)))
        }
    }
}

impl Device for NanoKvm {
    fn info(&self) -> DeviceInfo {
        DeviceInfo {
            vendor: Vendor::NanoKvm,
            model: "NanoKVM PCIe".to_string(),
            firmware: self.firmware.clone(),
            hardened: false,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            keyboard: true,
            absolute_mouse: true,
            relative_mouse: false, // only absolute (frame 0x02) observed so far
            video_mjpeg: true,
            video_h264: true,
            video_webrtc: false,
            power_on_off: true,
            power_reset: true, // real reset line via GPIO (type "reset")
            virtual_media: true,
            wake_on_lan: false,
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
        TransportKind::RequestResponse
    }
}

impl Hid for NanoKvm {
    async fn key(&self, event: KeyEvent) -> Result<()> {
        // Build the full HID report from tracked state (NanoKVM expects the whole
        // report each time), then send it. The lock is released before we await.
        let report = {
            let mut kb = self
                .keyboard
                .lock()
                .map_err(|_| Error::Protocol("keyboard state poisoned".into()))?;
            if event.pressed {
                kb.press(event.key.0);
            } else {
                kb.release(event.key.0);
            }
            kb.report()
        };
        self.send(report.to_vec()).await
    }

    async fn absolute_mouse(&self, m: AbsMouse) -> Result<()> {
        {
            let mut pos = self
                .mouse
                .lock()
                .map_err(|_| Error::Protocol("mouse state poisoned".into()))?;
            pos.x = m.x;
            pos.y = m.y;
        }
        self.send(mouse_frame(m.buttons, m.x, m.y, 0).to_vec())
            .await
    }

    async fn relative_mouse(&self, _m: RelMouse) -> Result<()> {
        // NanoKVM's observed protocol is absolute-only; relative needs more capture.
        Err(Error::NotSupported("relative mouse"))
    }

    async fn wheel(&self, w: Wheel) -> Result<()> {
        let (x, y) = {
            let pos = self
                .mouse
                .lock()
                .map_err(|_| Error::Protocol("mouse state poisoned".into()))?;
            (pos.x, pos.y)
        };
        self.send(mouse_frame(0, x, y, w.delta).to_vec()).await
    }
}

// Power and virtual media remain stubs for this slice.
impl Power for NanoKvm {
    async fn action(&self, action: PowerAction) -> Result<()> {
        // POST /api/vm/gpio {type, duration}: hold the power/reset line for `duration`
        // milliseconds. Valid types are "power" and "reset" (confirmed on the wire);
        // durations follow standard ATX timing (short tap vs. ~5s force-off long press).
        let (event, duration_ms) = match action {
            PowerAction::On | PowerAction::ShortPress => ("power", 100),
            PowerAction::LongPress => ("power", 5000),
            PowerAction::Reset => ("reset", 100),
        };
        self.post_json(
            "/api/vm/gpio",
            serde_json::json!({ "type": event, "duration": duration_ms }),
        )
        .await
        .map(|_| ())
    }

    async fn state(&self) -> Result<PowerState> {
        let d = self.get_data("/api/vm/gpio").await?;
        Ok(PowerState {
            powered: d["pwr"].as_bool(),
            hdd_activity: d["hdd"].as_bool(),
        })
    }
}

impl VirtualMedia for NanoKvm {
    async fn list(&self) -> Result<Vec<MediaImage>> {
        let data = self.get_data("/api/storage/image").await?;
        let mounted = self
            .get_data("/api/storage/image/mounted")
            .await
            .ok()
            .and_then(|m| m["file"].as_str().map(str::to_string));
        let mut out = Vec::new();
        if let Some(files) = data["files"].as_array() {
            for f in files {
                // Element shape when populated is unverified (device had no images at
                // capture time); accept a bare filename string or a {name,size} object.
                let name = f.as_str().or_else(|| f["name"].as_str());
                if let Some(name) = name {
                    out.push(MediaImage {
                        name: name.to_string(),
                        size: f["size"].as_u64(),
                        mounted: mounted.as_deref() == Some(name),
                    });
                }
            }
        }
        Ok(out)
    }

    async fn mount(&self, name: &str) -> Result<()> {
        match self
            .post_json(
                "/api/storage/image/mount",
                serde_json::json!({ "file": name }),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                // A refusal is opaque (`code -2`). When the device says its virtual disk
                // is off, say so: the user can fix it in the device settings.
                if let Ok(state) = self.get_data("/api/storage/media/state").await {
                    if virtual_disk_disabled(&state) {
                        return Err(Error::NotSupported(
                            "virtual disk is disabled on the device (enable it in its Device settings)",
                        ));
                    }
                }
                Err(e)
            }
        }
    }

    async fn unmount(&self) -> Result<()> {
        // An empty file unmounts (observed: posting no file returns "unmount ...").
        self.post_json(
            "/api/storage/image/mount",
            serde_json::json!({ "file": "" }),
        )
        .await
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_image_version, virtual_disk_disabled};

    #[test]
    fn recognises_a_disabled_virtual_disk() {
        let off = serde_json::json!({"state":"unsupported","type":"","rel":"","hidOnly":false,"open":[],"cdrom":false,"localWritable":true});
        let idle = serde_json::json!({"state":"partition","type":"partition","rel":"","hidOnly":false,"open":[],"cdrom":false,"localWritable":false});
        let mounted = serde_json::json!({"state":"image","type":"image","rel":"x.iso","hidOnly":false,"open":[],"cdrom":false,"localWritable":true});
        assert!(virtual_disk_disabled(&off));
        assert!(!virtual_disk_disabled(&idle));
        assert!(!virtual_disk_disabled(&mounted));
    }

    /// The exact `data` payload observed live from a NanoKVM PCIe's
    /// `GET /api/vm/info` (app 2.5.0, image v1.4.0).
    #[test]
    fn extracts_image_from_captured_shape() {
        let data = serde_json::json!({
            "ips": [{"name":"eth0","addr":"192.0.2.10","version":"IPv4","type":"Wired"}],
            "mdns": "",
            "image": "v1.4.0",
            "application": "2.5.0",
            "deviceKey": "0000000000000000"
        });
        assert_eq!(extract_image_version(&data), Some("v1.4.0".to_string()));
    }

    #[test]
    fn missing_image_field_is_none() {
        assert_eq!(extract_image_version(&serde_json::json!({})), None);
    }
}

#[cfg(test)]
mod quarantine_tests {
    use super::*;

    /// Password login must refuse before touching the network. `.invalid` can never
    /// resolve, so any attempt to connect would surface as a Transport error instead.
    #[tokio::test]
    async fn password_login_is_not_supported() {
        let err = NanoKvm::connect("nanokvm.invalid", "user", "pass")
            .await
            .err()
            .expect("password login must not succeed");
        assert!(matches!(err, Error::NotSupported(_)), "got {err:?}");
    }
}
