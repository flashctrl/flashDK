//! GL.iNet Comet (GL-RM1) adapter: `kvmd`-family REST API over real TLS
//! (self-signed, TOFU-pinned, same trust model as PiKVM).
//!
//! Derived entirely from live wire observation of a Comet unit this project owns
//! (see PROVENANCE.md and `docs/captures/glinet-comet-kvmd-api.md`), never from the
//! device's own served frontend bundle or any vendor source. HID uses the same
//! `/api/hid/events/*` endpoints PiKVM's `kvmd` does; power uses `/api/atx`; virtual
//! media `/api/msd`. The one real protocol difference from PiKVM: this device
//! authenticates via a login exchange (`/api/auth/login`) returning a session
//! token, sent back as a `Token` header, rather than PiKVM's static per-request
//! `X-KVMD-User`/`X-KVMD-Passwd` headers.

use std::sync::Arc;

use flashdk_core::capability::Vendor;
use flashdk_core::hid::{AbsMouse, Hid, KeyEvent, RelMouse, Wheel};
use flashdk_core::media::{MediaImage, VirtualMedia};
use flashdk_core::power::{Power, PowerAction, PowerState};
use flashdk_core::{Capabilities, Device, DeviceInfo, Error, Result, TransportKind};
use reqwest::RequestBuilder;
use tokio::sync::Mutex;

use crate::pikvm::keymap;
use crate::tls_pin::{self, MemoryPinStore, PinStore};

/// kvmd's uniform reply shape: `{"ok": bool, "result": {...}}`, confirmed live for
/// this device the same way it's confirmed for PiKVM.
#[derive(serde::Deserialize)]
struct ApiResponse {
    ok: bool,
    #[serde(default)]
    result: serde_json::Value,
}

/// A connected GL.iNet Comet.
pub struct GlInetKvm {
    base_url: String,
    http: reqwest::Client,
    /// Session token from `/api/auth/login`, sent back as a bare `Token` header.
    /// `None` before login or after `logout()`.
    token: Mutex<Option<String>>,
    identity: Mutex<Option<(String, String)>>,
}

impl GlInetKvm {
    /// Log in to `host` (e.g. `"10.0.10.22"`) with the Comet's admin credentials.
    ///
    /// The Comet ships a self-signed certificate like PiKVM does, so this uses the
    /// same trust-on-first-use pinning; see [`crate::tls_pin`].
    pub async fn connect(host: &str, user: &str, passwd: &str) -> Result<Self> {
        Self::connect_with_pin_store(host, user, passwd, Arc::new(MemoryPinStore::default())).await
    }

    /// Like [`Self::connect`], but pins are read from and written to `store`.
    pub async fn connect_with_pin_store(
        host: &str,
        user: &str,
        passwd: &str,
        store: Arc<dyn PinStore>,
    ) -> Result<Self> {
        let host_owned = host.to_string();
        let http = tls_pin::tofu_client(&host_owned, store).map_err(Error::Transport)?;
        let mut kvm = Self {
            base_url: format!("https://{host_owned}"),
            http,
            token: Mutex::new(None),
            identity: Mutex::new(None),
        };
        kvm.login(user, passwd).await?;
        kvm.refresh_identity().await?;
        Ok(kvm)
    }

    /// `POST /api/auth/login`, form-encoded `user`/`passwd` (a JSON body is
    /// rejected; captured live, see PROVENANCE.md). Stores the returned `token` for
    /// use as a `Token` header on every later request.
    async fn login(&mut self, user: &str, passwd: &str) -> Result<()> {
        let resp: ApiResponse = self
            .http
            .post(format!("{}/api/auth/login", self.base_url))
            .form(&[("user", user), ("passwd", passwd)])
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?
            .json()
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        if !resp.ok {
            return Err(Error::Auth(resp.result.to_string()));
        }
        let token = resp.result["token"]
            .as_str()
            .ok_or_else(|| Error::Auth("login response had no token".into()))?
            .to_string();
        *self.token.lock().await = Some(token);
        Ok(())
    }

    /// `POST /api/auth/logout`, invalidating the current session token.
    pub async fn logout(&self) -> Result<()> {
        self.post("/api/auth/logout", &[]).await?;
        *self.token.lock().await = None;
        Ok(())
    }

    /// Fetch model/firmware from `/api/info` and cache them, the same
    /// already-connected-so-fetch-inline pattern NanoKVM/JetKVM use (this
    /// constructor is already async, unlike PiKVM's sync one).
    async fn refresh_identity(&self) -> Result<()> {
        let r = self.get("/api/info").await?;
        let model = r["system"]["platform"]["base"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| "GL.iNet Comet".to_string());
        let firmware = r["system"]["kvmd"]["version"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| "unknown".to_string());
        if let Ok(mut id) = self.identity.try_lock() {
            *id = Some((model, firmware));
        }
        Ok(())
    }

    /// Attach the `Token` header this device expects (not `Authorization: Bearer`,
    /// which was tried live and rejected; see PROVENANCE.md).
    async fn auth(&self, rb: RequestBuilder) -> RequestBuilder {
        match &*self.token.lock().await {
            Some(token) => rb.header("Token", token),
            None => rb,
        }
    }

    fn check(api: ApiResponse) -> Result<serde_json::Value> {
        if api.ok {
            Ok(api.result)
        } else {
            Err(Error::Protocol(api.result.to_string()))
        }
    }

    async fn get(&self, path: &str) -> Result<serde_json::Value> {
        let rb = self.http.get(format!("{}{}", self.base_url, path));
        let resp = self
            .auth(rb)
            .await
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        Self::check(
            resp.json()
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?,
        )
    }

    /// POST a path with query params (matching this device's convention, verified
    /// live for `/api/hid/events/*` and `/api/atx/click`).
    async fn post(&self, path: &str, params: &[(&str, &str)]) -> Result<()> {
        let rb = self
            .http
            .post(format!("{}{}", self.base_url, path))
            .query(params);
        let resp = self
            .auth(rb)
            .await
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

    /// Reconcile the three mouse buttons to kvmd's per-button events, the same
    /// approach PiKVM's adapter uses (this device's `send_mouse_button` shape is
    /// independently verified live for `left`; `right`/`middle` follow the same
    /// documented kvmd-family convention).
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

impl Device for GlInetKvm {
    fn info(&self) -> DeviceInfo {
        let (model, firmware) = self
            .identity
            .try_lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_else(|| ("GL.iNet Comet".to_string(), "unknown".to_string()));
        DeviceInfo {
            vendor: Vendor::GlInet,
            model,
            firmware,
            hardened: false,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            keyboard: true,
            absolute_mouse: true,
            // kvmd's `mouse.outputs` on this device lists a `usb_rel` mode
            // alongside the default `usb` (absolute) one, but switching output
            // modes wasn't captured, so relative mouse stays unimplemented rather
            // than claimed here; see PROVENANCE.md.
            relative_mouse: false,
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

impl Hid for GlInetKvm {
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
        // Same linear map PiKVM's adapter uses (core's 0..=32767 -> kvmd's roughly
        // -32768..=32767 per axis). Not independently re-derived against this
        // specific device (no host attached to observe a cursor land); see
        // PROVENANCE.md for why this is a documented assumption, not a fact.
        let to_x = ((m.x as i32) * 2 - 32768).to_string();
        let to_y = ((m.y as i32) * 2 - 32768).to_string();
        self.post(
            "/api/hid/events/send_mouse_move",
            &[("to_x", &to_x), ("to_y", &to_y)],
        )
        .await?;
        self.sync_buttons(m.buttons).await
    }

    async fn relative_mouse(&self, _m: RelMouse) -> Result<()> {
        Err(Error::NotSupported(
            "relative mouse mode switch not yet captured on this device",
        ))
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

impl Power for GlInetKvm {
    async fn action(&self, action: PowerAction) -> Result<()> {
        // Button names confirmed live: power, power_long, and reset all return
        // ok:true against the real device (see docs/captures).
        match action {
            PowerAction::On => self.post("/api/atx/click", &[("button", "power")]).await,
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

impl VirtualMedia for GlInetKvm {
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
        self.post("/api/msd/set_params", &[("image", name)]).await?;
        self.post("/api/msd/set_connected", &[("connected", "1")])
            .await
    }

    async fn unmount(&self) -> Result<()> {
        self.post("/api/msd/set_connected", &[("connected", "0")])
            .await
    }
}
