//! JetKVM WebRTC transport (sans-IO via str0m).
//!
//! JetKVM speaks [`TransportKind::PeerRpc`](flashdk_core::TransportKind): control only
//! works over a WebRTC peer connection. [`connect`] performs the full lifecycle —
//! build an offer with the device's data channels, exchange SDP over
//! `POST /webrtc/session`, then drive str0m's sans-IO loop on a background task over a
//! UDP socket. It returns a [`JetTransport`] handle: HID frames go over the binary
//! `hidrpc*` channels, and control calls over the JSON-RPC `rpc` channel.

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use flashdk_core::{Error, Result};
use str0m::channel::ChannelId;
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, Input, Output, Rtc};
use tokio::sync::{mpsc, oneshot};

/// The data-channel labels JetKVM uses (observed on the wire).
pub const CH_RPC: &str = "rpc";
pub const CH_HID: &str = "hidrpc";
pub const CH_HID_UNRELIABLE_ORDERED: &str = "hidrpc-unreliable-ordered";

/// A command sent to the driver task.
enum Cmd {
    /// Write a binary HID frame to a data channel.
    Write { channel: ChannelId, data: Vec<u8> },
    /// Issue a JSON-RPC request on the `rpc` channel, correlating the reply by `id`.
    Rpc {
        id: String,
        request: Vec<u8>,
        resp: oneshot::Sender<Result<serde_json::Value>>,
    },
}

/// Handle to a live JetKVM WebRTC connection. HID frames route to the binary channels;
/// [`rpc_call`](JetTransport::rpc_call) issues control requests over the `rpc` channel.
pub struct JetTransport {
    cmd_tx: mpsc::UnboundedSender<Cmd>,
    hid: ChannelId,
    hid_unreliable: ChannelId,
    next_id: AtomicU64,
}

impl JetTransport {
    /// Send a reliable HID frame (keyboard) on the `hidrpc` channel.
    pub fn send_hid(&self, data: Vec<u8>) -> Result<()> {
        self.cmd_tx
            .send(Cmd::Write {
                channel: self.hid,
                data,
            })
            .map_err(|_| Error::Transport("jetkvm driver stopped".into()))
    }

    /// Send a low-latency HID frame (mouse) on the `hidrpc-unreliable-ordered` channel.
    pub fn send_hid_unreliable(&self, data: Vec<u8>) -> Result<()> {
        self.cmd_tx
            .send(Cmd::Write {
                channel: self.hid_unreliable,
                data,
            })
            .map_err(|_| Error::Transport("jetkvm driver stopped".into()))
    }

    /// Issue a JSON-RPC 2.0 request on the `rpc` channel and await the result.
    pub async fn rpc_call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let id = format!("flashdk-{}", self.next_id.fetch_add(1, Ordering::Relaxed));
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": id,
        })
        .to_string()
        .into_bytes();
        let (resp, rx) = oneshot::channel();
        self.cmd_tx
            .send(Cmd::Rpc { id, request, resp })
            .map_err(|_| Error::Transport("jetkvm driver stopped".into()))?;
        rx.await
            .map_err(|_| Error::Transport("jetkvm driver dropped rpc".into()))?
    }
}

/// Establish the WebRTC connection to `host` (already-authenticated `http` client for
/// signaling), returning a handle once the `hidrpc` channel is open.
pub async fn connect(http: reqwest::Client, host: &str) -> Result<JetTransport> {
    // 1) Bind the media socket and determine our routable local address.
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| Error::Transport(e.to_string()))?;
    socket
        .set_nonblocking(true)
        .map_err(|e| Error::Transport(e.to_string()))?;
    let local_addr = routable_local_addr(host, &socket)?;

    // 2) Build the offerer Rtc: add our host candidate, then the data channels.
    let mut rtc = Rtc::new();
    let candidate =
        Candidate::host(local_addr, Protocol::Udp).map_err(|e| Error::Transport(e.to_string()))?;
    rtc.add_local_candidate(candidate);

    let mut api = rtc.sdp_api();
    let rpc = api.add_channel(CH_RPC.to_string());
    let hid = api.add_channel(CH_HID.to_string());
    let hid_unreliable = api.add_channel(CH_HID_UNRELIABLE_ORDERED.to_string());
    let (offer, pending) = api
        .apply()
        .ok_or_else(|| Error::Protocol("no offer produced".into()))?;

    // 3) Exchange SDP with the device and accept the answer.
    let base_url = format!("http://{host}");
    let answer_sdp = exchange_sdp(&http, &base_url, &offer.to_sdp_string()).await?;
    let answer = str0m::change::SdpAnswer::from_sdp_string(&answer_sdp)
        .map_err(|e| Error::Protocol(format!("bad answer SDP: {e}")))?;
    rtc.sdp_api()
        .accept_answer(pending, answer)
        .map_err(|e| Error::Protocol(format!("accept_answer: {e}")))?;

    // 4) Spawn the driver; wait until the hidrpc channel opens.
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (ready_tx, ready_rx) = oneshot::channel();
    let tokio_socket =
        tokio::net::UdpSocket::from_std(socket).map_err(|e| Error::Transport(e.to_string()))?;
    tokio::spawn(drive(
        rtc,
        tokio_socket,
        local_addr,
        cmd_rx,
        Some(ready_tx),
        hid,
        rpc,
    ));

    ready_rx
        .await
        .map_err(|_| Error::Transport("driver ended before hidrpc opened".into()))??;

    Ok(JetTransport {
        cmd_tx,
        hid,
        hid_unreliable,
        next_id: AtomicU64::new(1),
    })
}

/// Discover the local source address that routes to `host` (so our ICE host candidate
/// advertises a reachable IP), reusing the media socket's port.
fn routable_local_addr(host: &str, media: &UdpSocket) -> Result<SocketAddr> {
    let probe = UdpSocket::bind("0.0.0.0:0").map_err(|e| Error::Transport(e.to_string()))?;
    probe
        .connect((host, 80))
        .map_err(|e| Error::Transport(e.to_string()))?;
    let ip = probe
        .local_addr()
        .map_err(|e| Error::Transport(e.to_string()))?
        .ip();
    let port = media
        .local_addr()
        .map_err(|e| Error::Transport(e.to_string()))?
        .port();
    Ok(SocketAddr::new(ip, port))
}

/// The sans-IO event loop: pump str0m's outputs, feed inbound datagrams and commands,
/// and correlate JSON-RPC replies on the `rpc` channel.
#[allow(clippy::too_many_arguments)]
async fn drive(
    mut rtc: Rtc,
    socket: tokio::net::UdpSocket,
    local_addr: SocketAddr,
    mut cmd_rx: mpsc::UnboundedReceiver<Cmd>,
    mut ready_tx: Option<oneshot::Sender<Result<()>>>,
    hid: ChannelId,
    rpc: ChannelId,
) {
    let mut buf = vec![0u8; 2048];
    let mut pending: HashMap<String, oneshot::Sender<Result<serde_json::Value>>> = HashMap::new();
    loop {
        // Drain outputs until str0m asks us to wait.
        let timeout = loop {
            match rtc.poll_output() {
                Ok(Output::Timeout(t)) => break t,
                Ok(Output::Transmit(t)) => {
                    let _ = socket.send_to(&t.contents, t.destination).await;
                }
                Ok(Output::Event(ev)) => {
                    match ev {
                        Event::ChannelOpen(id, _label) if id == hid => {
                            if let Some(tx) = ready_tx.take() {
                                let _ = tx.send(Ok(()));
                            }
                        }
                        Event::ChannelData(cd) if cd.id == rpc => {
                            dispatch_rpc(&cd.data, &mut pending);
                        }
                        _ => {}
                    }
                    if !rtc.is_alive() {
                        return;
                    }
                }
                Err(_) => {
                    if let Some(tx) = ready_tx.take() {
                        let _ = tx.send(Err(Error::Transport("rtc error".into())));
                    }
                    return;
                }
            }
        };

        let wait = timeout.saturating_duration_since(Instant::now());
        tokio::select! {
            _ = tokio::time::sleep(wait) => {
                let _ = rtc.handle_input(Input::Timeout(Instant::now()));
            }
            r = socket.recv_from(&mut buf) => {
                if let Ok((n, from)) = r {
                    if let Ok(contents) = (&buf[..n]).try_into() {
                        let _ = rtc.handle_input(Input::Receive(
                            Instant::now(),
                            Receive { proto: Protocol::Udp, source: from, destination: local_addr, contents },
                        ));
                    }
                }
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(Cmd::Write { channel, data }) => {
                        if let Some(mut ch) = rtc.channel(channel) {
                            let _ = ch.write(true, &data);
                        }
                    }
                    Some(Cmd::Rpc { id, request, resp }) => {
                        match rtc.channel(rpc) {
                            Some(mut ch) => {
                                pending.insert(id, resp);
                                let _ = ch.write(false, &request); // JSON text
                            }
                            None => { let _ = resp.send(Err(Error::Transport("rpc channel not open".into()))); }
                        }
                    }
                    None => return, // handle dropped
                }
            }
        }
    }
}

/// Parse a JSON-RPC reply and fulfil the matching pending request.
fn dispatch_rpc(
    data: &[u8],
    pending: &mut HashMap<String, oneshot::Sender<Result<serde_json::Value>>>,
) {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };
    // id may be a string or a number; normalise to string for correlation.
    let id = match &v["id"] {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => return,
    };
    if let Some(tx) = pending.remove(&id) {
        let result = if v.get("error").is_some_and(|e| !e.is_null()) {
            Err(Error::Protocol(format!("rpc error: {}", v["error"])))
        } else {
            Ok(v.get("result").cloned().unwrap_or(serde_json::Value::Null))
        };
        let _ = tx.send(result);
    }
}

/// Exchange an SDP offer with the device and return its SDP answer.
///
/// JetKVM's `POST /webrtc/session` body is a JSON object `{"sd": "<base64>"}` where the
/// base64 decodes to a JSON session description `{"type","sdp"}`; the reply is the same
/// shape carrying the answer. (Established by probing + live iteration — see
/// docs/captures/jetkvm-datachannel-hid.md.)
pub async fn exchange_sdp(
    http: &reqwest::Client,
    base_url: &str,
    offer_sdp: &str,
) -> Result<String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;

    let inner = serde_json::json!({ "type": "offer", "sdp": offer_sdp }).to_string();
    let body = serde_json::json!({ "sd": b64.encode(inner) }).to_string();

    let resp = http
        .post(format!("{base_url}/webrtc/session"))
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| Error::Transport(e.to_string()))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| Error::Transport(e.to_string()))?;
    if !status.is_success() {
        return Err(Error::Protocol(format!(
            "signaling failed: HTTP {status}: {}",
            text.chars().take(200).collect::<String>()
        )));
    }

    let outer: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| Error::Protocol(e.to_string()))?;
    let sd = outer["sd"]
        .as_str()
        .ok_or_else(|| Error::Protocol("answer missing 'sd'".into()))?;
    let decoded = b64
        .decode(sd.trim())
        .map_err(|e| Error::Protocol(format!("answer 'sd' not base64: {e}")))?;
    let inner: serde_json::Value =
        serde_json::from_slice(&decoded).map_err(|e| Error::Protocol(e.to_string()))?;
    inner["sdp"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| Error::Protocol("answer missing 'sdp'".into()))
}
