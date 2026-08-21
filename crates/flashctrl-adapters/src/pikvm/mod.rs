//! PiKVM v3 adapter — REST + WebSocket over real TLS (HSTS, self-signed CN=localhost).
//!
//! This is the first adapter with **real** HID behaviour. Everything here was derived
//! from wire observation of kvmd 4.206 (see PROVENANCE.md and
//! docs/captures/pikvm-hid-rest.md) — never from source.
//!
//! HID uses kvmd's REST event endpoints under `/api/hid/events/`. Auth is the simple
//! header scheme (`X-KVMD-User` / `X-KVMD-Passwd`) sent on every request; a nicer
//! cookie session and the low-latency WebSocket path come later.

mod keymap;

use flashctrl_core::capability::Vendor;
use flashctrl_core::hid::{AbsMouse, Hid, KeyEvent, RelMouse, Wheel};
use flashctrl_core::media::{MediaImage, VirtualMedia};
use flashctrl_core::power::{Power, PowerAction, PowerState};
use flashctrl_core::{Capabilities, Device, DeviceInfo, Error, Result, TransportKind};

/// kvmd's uniform reply shape: `{"ok": bool, "result": {...}}`.
#[derive(serde::Deserialize)]
struct ApiResponse {
    ok: bool,
    #[serde(default)]
    result: serde_json::Value,
}

/// A connected PiKVM.
pub struct PiKvm {
    base_url: String,
    user: String,
    passwd: String,
    http: reqwest::Client,
}

impl PiKvm {
    /// Point an adapter at `host` (e.g. "10.0.10.20") with kvmd credentials.
    ///
    /// We currently accept the device's self-signed certificate. That's a deliberate
    /// stopgap: the real plan (see `Capabilities::tls_pinnable`) is trust-on-first-use
    /// pinning of PiKVM's certificate. Until that lands, treat this as LAN-only.
    pub fn new(
        host: impl Into<String>,
        user: impl Into<String>,
        passwd: impl Into<String>,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(true) // TODO: replace with TOFU cert pinning
            .build()
            .map_err(|e| Error::Transport(e.to_string()))?;
        Ok(Self {
            base_url: format!("https://{}", host.into()),
            user: user.into(),
            passwd: passwd.into(),
            http,
        })
    }

    /// POST to a `/api/hid/events/<endpoint>?<query>` and check kvmd's `ok` flag.
    async fn post_event(&self, endpoint: &str, query: &str) -> Result<()> {
        let url = format!("{}/api/hid/events/{}?{}", self.base_url, endpoint, query);
        let resp = self
            .http
            .post(&url)
            .header("X-KVMD-User", &self.user)
            .header("X-KVMD-Passwd", &self.passwd)
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        let api: ApiResponse = resp
            .json()
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;
        if api.ok {
            Ok(())
        } else {
            Err(Error::Protocol(api.result.to_string()))
        }
    }

    /// Reconcile the three mouse buttons to a bitmask (bit0 left, bit1 right, bit2 middle).
    /// Stateless for now — a later WebSocket path will diff and only send changes.
    async fn sync_buttons(&self, mask: u8) -> Result<()> {
        for (bit, name) in [(0u8, "left"), (1, "right"), (2, "middle")] {
            let pressed = mask & (1u8 << bit) != 0;
            self.post_event("send_mouse_button", &format!("button={name}&state={pressed}"))
                .await?;
        }
        Ok(())
    }
}

impl Device for PiKvm {
    fn info(&self) -> DeviceInfo {
        DeviceInfo {
            vendor: Vendor::PiKvm,
            model: "PiKVM v3".to_string(),
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
            video_webrtc: true,
            power_on_off: true,
            power_reset: true,
            virtual_media: true,
            wake_on_lan: false,
            tls_pinnable: true,
        }
    }

    fn transport_kind(&self) -> TransportKind {
        TransportKind::RequestResponse
    }
}

impl Hid for PiKvm {
    async fn key(&self, event: KeyEvent) -> Result<()> {
        // Translate our standard USB HID usage id into kvmd's W3C code name.
        let code = keymap::usage_to_code(event.key.0)
            .ok_or(Error::NotSupported("key not in keymap"))?;
        self.post_event("send_key", &format!("key={code}&state={}", event.pressed))
            .await
    }

    async fn absolute_mouse(&self, m: AbsMouse) -> Result<()> {
        // Core is 0..=32767 per axis; kvmd wants roughly -32768..=32767. Linear-map it.
        let to_x = (m.x as i32) * 2 - 32768;
        let to_y = (m.y as i32) * 2 - 32768;
        self.post_event("send_mouse_move", &format!("to_x={to_x}&to_y={to_y}"))
            .await?;
        self.sync_buttons(m.buttons).await
    }

    async fn relative_mouse(&self, m: RelMouse) -> Result<()> {
        self.post_event(
            "send_mouse_relative",
            &format!("delta_x={}&delta_y={}", m.dx, m.dy),
        )
        .await?;
        self.sync_buttons(m.buttons).await
    }

    async fn wheel(&self, w: Wheel) -> Result<()> {
        // kvmd takes both axes; we drive vertical.
        self.post_event("send_mouse_wheel", &format!("delta_x=0&delta_y={}", w.delta))
            .await
    }
}

// Power and virtual media stay stubs for this slice — HID first.
impl Power for PiKvm {
    async fn action(&self, _action: PowerAction) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn state(&self) -> Result<PowerState> {
        Err(Error::NotImplemented)
    }
}

impl VirtualMedia for PiKvm {
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
