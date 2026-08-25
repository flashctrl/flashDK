//! PiKVM v3 adapter — REST + WebSocket over real TLS (HSTS, self-signed CN=localhost).
//!
//! Derived from wire observation of kvmd 4.206 and PiKVM's official API docs (see
//! PROVENANCE.md and docs/captures/pikvm-hid-rest.md) — never from source. HID uses
//! the `/api/hid/events/` endpoints; power uses `/api/atx`; virtual media `/api/msd`.
//! Auth is the header scheme (`X-KVMD-User` / `X-KVMD-Passwd`) on every request.

mod keymap;

use std::sync::Arc;

use flashdk_core::capability::Vendor;
use flashdk_core::hid::{AbsMouse, Hid, KeyEvent, RelMouse, Wheel};
use flashdk_core::media::{MediaImage, VirtualMedia};
use flashdk_core::power::{Power, PowerAction, PowerState};
use flashdk_core::{Capabilities, Device, DeviceInfo, Error, Result, TransportKind};
use reqwest::RequestBuilder;

use crate::tls_pin::{self, MemoryPinStore, PinStore};

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
    /// Cached (model, firmware), populated by [`Self::refresh_identity`]. `new()`
    /// makes no network call (matching `Device::info`'s "cheap, no I/O" contract), so
    /// this starts `None` and `info()` falls back to sensible defaults until a caller
    /// refreshes it.
    identity: std::sync::Mutex<Option<(String, String)>>,
}

impl PiKvm {
    /// Point an adapter at `host` (e.g. "10.0.10.20") with kvmd credentials.
    ///
    /// PiKVM ships a self-signed certificate, so ordinary CA-chain validation can
    /// never succeed. Rather than disabling certificate checking outright (which
    /// would accept *any* certificate from *anyone*), this pins the certificate seen
    /// on first connect and rejects a *different* one later — see [`crate::tls_pin`].
    /// Pins are kept in memory for this instance; call [`Self::with_pin_store`] to
    /// supply persistent storage (Keychain/Keystore) so a pin survives restarts.
    pub fn new(
        host: impl Into<String>,
        user: impl Into<String>,
        passwd: impl Into<String>,
    ) -> Result<Self> {
        Self::with_pin_store(host, user, passwd, Arc::new(MemoryPinStore::default()))
    }

    /// Like [`Self::new`], but pins are read from and written to `store` — supply a
    /// persistent implementation to remember a device across app restarts.
    pub fn with_pin_store(
        host: impl Into<String>,
        user: impl Into<String>,
        passwd: impl Into<String>,
        store: Arc<dyn PinStore>,
    ) -> Result<Self> {
        let host = host.into();
        let http = tls_pin::tofu_client(&host, store).map_err(Error::Transport)?;
        Ok(Self {
            base_url: format!("https://{host}"),
            user: user.into(),
            passwd: passwd.into(),
            http,
            identity: std::sync::Mutex::new(None),
        })
    }

    /// Fetch the board model and kvmd version from `/api/info` and cache them so
    /// subsequent [`Device::info`] calls report real values instead of the defaults.
    /// `new()` deliberately makes no network call, so call this once after connecting
    /// if you want accurate identity (e.g. for a "hardened firmware ✓" badge later).
    pub async fn refresh_identity(&self) -> Result<()> {
        let r = self.get("/api/info").await?;
        let model = r["hw"]["platform"]["base"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| "PiKVM".to_string());
        let firmware = r["system"]["kvmd"]["version"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| "unknown".to_string());
        if let Ok(mut id) = self.identity.lock() {
            *id = Some((model, firmware));
        }
        Ok(())
    }

    /// Attach the auth headers kvmd expects to any request.
    fn auth(&self, rb: RequestBuilder) -> RequestBuilder {
        rb.header("X-KVMD-User", &self.user)
            .header("X-KVMD-Passwd", &self.passwd)
    }

    /// Inspect kvmd's `ok` flag, returning the `result` value or an error.
    fn check(api: ApiResponse) -> Result<serde_json::Value> {
        if api.ok {
            Ok(api.result)
        } else {
            Err(Error::Protocol(api.result.to_string()))
        }
    }

    /// GET a path and return its `result` payload.
    async fn get(&self, path: &str) -> Result<serde_json::Value> {
        let resp = self
            .auth(self.http.get(format!("{}{}", self.base_url, path)))
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        Self::check(
            resp.json()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
        )
    }

    /// POST a path with query params (properly URL-encoded), ignoring the payload.
    async fn post(&self, path: &str, params: &[(&str, &str)]) -> Result<()> {
        let resp = self
            .auth(
                self.http
                    .post(format!("{}{}", self.base_url, path))
                    .query(params),
            )
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        Self::check(
            resp.json()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
        )?;
        Ok(())
    }

    /// Reconcile the three mouse buttons to a bitmask (bit0 left, bit1 right, bit2 middle).
    /// Stateless for now — a later WebSocket path will diff and only send changes.
    async fn sync_buttons(&self, mask: u8) -> Result<()> {
        for (bit, name) in [(0u8, "left"), (1, "right"), (2, "middle")] {
            let state = if mask & (1u8 << bit) != 0 {
                "true"
            } else {
                "false"
            };
            self.post(
                "/api/hid/events/send_mouse_button",
                &[("button", name), ("state", state)],
            )
            .await?;
        }
        Ok(())
    }
}

impl Device for PiKvm {
    fn info(&self) -> DeviceInfo {
        let (model, firmware) = self
            .identity
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_else(|| ("PiKVM v3".to_string(), "unknown".to_string()));
        DeviceInfo {
            vendor: Vendor::PiKvm,
            model,
            firmware,
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
        let code =
            keymap::usage_to_code(event.key.0).ok_or(Error::NotSupported("key not in keymap"))?;
        let state = if event.pressed { "true" } else { "false" };
        self.post(
            "/api/hid/events/send_key",
            &[("key", code), ("state", state)],
        )
        .await
    }

    async fn absolute_mouse(&self, m: AbsMouse) -> Result<()> {
        // Core is 0..=32767 per axis; kvmd wants roughly -32768..=32767. Linear-map it.
        let to_x = ((m.x as i32) * 2 - 32768).to_string();
        let to_y = ((m.y as i32) * 2 - 32768).to_string();
        self.post(
            "/api/hid/events/send_mouse_move",
            &[("to_x", &to_x), ("to_y", &to_y)],
        )
        .await?;
        self.sync_buttons(m.buttons).await
    }

    async fn relative_mouse(&self, m: RelMouse) -> Result<()> {
        let (dx, dy) = (m.dx.to_string(), m.dy.to_string());
        self.post(
            "/api/hid/events/send_mouse_relative",
            &[("delta_x", &dx), ("delta_y", &dy)],
        )
        .await?;
        self.sync_buttons(m.buttons).await
    }

    async fn wheel(&self, w: Wheel) -> Result<()> {
        let d = w.delta.to_string();
        self.post(
            "/api/hid/events/send_mouse_wheel",
            &[("delta_x", "0"), ("delta_y", &d)],
        )
        .await
    }
}

impl Power for PiKvm {
    async fn action(&self, action: PowerAction) -> Result<()> {
        // Action values from PiKVM's official API docs; parameter names confirmed on
        // the wire. `On` uses atx/power; the presses/reset use atx/click.
        match action {
            PowerAction::On => self.post("/api/atx/power", &[("action", "on")]).await,
            PowerAction::ShortPress => self.post("/api/atx/click", &[("button", "power")]).await,
            PowerAction::LongPress => {
                self.post("/api/atx/click", &[("button", "power_long")])
                    .await
            }
            PowerAction::Reset => self.post("/api/atx/click", &[("button", "reset")]).await,
        }
    }

    async fn state(&self) -> Result<PowerState> {
        let r = self.get("/api/atx").await?;
        Ok(PowerState {
            powered: r["leds"]["power"].as_bool(),
            hdd_activity: r["leds"]["hdd"].as_bool(),
        })
    }
}

impl VirtualMedia for PiKvm {
    async fn list(&self) -> Result<Vec<MediaImage>> {
        let r = self.get("/api/msd").await?;
        let current = r["drive"]["image"].as_str();
        let connected = r["drive"]["connected"].as_bool().unwrap_or(false);
        let mut out = Vec::new();
        if let Some(images) = r["storage"]["images"].as_object() {
            for (name, info) in images {
                out.push(MediaImage {
                    name: name.clone(),
                    size: info["size"].as_u64(),
                    mounted: connected && current == Some(name.as_str()),
                });
            }
        }
        Ok(out)
    }

    async fn mount(&self, name: &str) -> Result<()> {
        // Select the image, then connect the emulated drive.
        self.post("/api/msd/set_params", &[("image", name)]).await?;
        self.post("/api/msd/set_connected", &[("connected", "1")])
            .await
    }

    async fn unmount(&self) -> Result<()> {
        self.post("/api/msd/set_connected", &[("connected", "0")])
            .await
    }
}
