//! NanoKVM PCIe adapter — REST for auth/aux, and a binary WebSocket (`/api/ws`) for
//! live keyboard and mouse. Everything here is derived from wire observation
//! (docs/captures/nanokvm-ws-hid.md, and the publicly-documented AES login).

mod auth;
mod wire;

use flashdk_core::capability::Vendor;
use flashdk_core::hid::{AbsMouse, Hid, KeyEvent, RelMouse, Wheel};
use flashdk_core::media::{MediaImage, VirtualMedia};
use flashdk_core::power::{Power, PowerAction, PowerState};
use flashdk_core::{Capabilities, Device, DeviceInfo, Error, Result, TransportKind};

use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use std::sync::Mutex;
use tokio::net::TcpStream;
use tokio::sync::Mutex as AsyncMutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use wire::{mouse_frame, KeyboardState};

type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

/// Last-known absolute cursor position, so wheel/button-only events can re-send it.
#[derive(Default, Clone, Copy)]
struct MousePos {
    x: u16,
    y: u16,
}

/// A connected NanoKVM.
pub struct NanoKvm {
    host: String,
    ws: AsyncMutex<WsSink>,
    keyboard: Mutex<KeyboardState>,
    mouse: Mutex<MousePos>,
}

impl NanoKvm {
    /// Log in and open the HID WebSocket. `host` is e.g. "10.0.10.10".
    ///
    /// Uses plaintext `http`/`ws` today; the device also offers TLS on 443, and a
    /// later pass will prefer `wss` with certificate handling.
    pub async fn connect(host: impl Into<String>, username: &str, password: &str) -> Result<Self> {
        let host = host.into();
        let base_url = format!("http://{host}");

        let http = reqwest::Client::builder()
            .build()
            .map_err(|e| Error::Transport(e.to_string()))?;
        let token = auth::login(&http, &base_url, username, password).await?;

        // Open the HID WebSocket, authenticating with the token cookie.
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        use tokio_tungstenite::tungstenite::http::header::COOKIE;
        let mut req = format!("ws://{host}/api/ws")
            .into_client_request()
            .map_err(|e| Error::Transport(e.to_string()))?;
        req.headers_mut().insert(
            COOKIE,
            format!("nano-kvm-token={token}")
                .parse()
                .map_err(|_| Error::Protocol("bad cookie header".into()))?,
        );
        let (stream, _resp) = tokio_tungstenite::connect_async(req)
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        let (sink, _read) = stream.split();

        Ok(Self {
            host,
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
        sink.send(Message::Binary(bytes))
            .await
            .map_err(|e| Error::Transport(e.to_string()))
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
            relative_mouse: false, // only absolute (frame 0x02) observed so far
            video_mjpeg: true,
            video_h264: true,
            video_webrtc: false,
            power_on_off: true,
            power_reset: false,
            virtual_media: true,
            wake_on_lan: false,
            tls_pinnable: false,
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
