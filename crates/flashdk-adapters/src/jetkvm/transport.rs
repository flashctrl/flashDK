//! JetKVM WebRTC transport (sans-IO via str0m).
//!
//! JetKVM speaks [`TransportKind::PeerRpc`](flashdk_core::TransportKind): control only
//! works over a WebRTC peer connection. [`connect`] performs the full lifecycle —
//! build an offer with the device's data channels, exchange SDP over
//! `POST /webrtc/session`, then drive str0m's sans-IO loop on a background task over a
//! UDP socket. It returns a [`JetTransport`] handle; HID frames are sent by posting
//! commands to the driver, which writes them to the open channels.

#![allow(dead_code)] // wired into the adapter incrementally

use std::net::{SocketAddr, UdpSocket};
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

/// A command sent to the driver task: write `data` to a data channel.
enum Cmd {
    Write { channel: ChannelId, data: Vec<u8> },
}

/// Handle to a live JetKVM WebRTC connection. Cloneable senders route HID frames to
/// the background driver that owns the `Rtc`.
pub struct JetTransport {
    cmd_tx: mpsc::UnboundedSender<Cmd>,
    hid: ChannelId,
    hid_unreliable: ChannelId,
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
}

/// Establish the WebRTC connection to `host` (already-authenticated `http` client for
/// signaling), returning a handle once the `hidrpc` channel is open.
pub async fn connect(http: reqwest::Client, host: &str) -> Result<JetTransport> {
    // 1) Bind the media socket and determine our routable local address (the source IP
    //    the device will see) via a scratch connect to the host.
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
    let _rpc = api.add_channel(CH_RPC.to_string());
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
    ));

    ready_rx
        .await
        .map_err(|_| Error::Transport("driver ended before hidrpc opened".into()))??;

    Ok(JetTransport {
        cmd_tx,
        hid,
        hid_unreliable,
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

/// The sans-IO event loop: pump str0m's outputs, feed inbound datagrams and commands.
async fn drive(
    mut rtc: Rtc,
    socket: tokio::net::UdpSocket,
    local_addr: SocketAddr,
    mut cmd_rx: mpsc::UnboundedReceiver<Cmd>,
    mut ready_tx: Option<oneshot::Sender<Result<()>>>,
    hid: ChannelId,
) {
    let mut buf = vec![0u8; 2048];
    loop {
        // Drain outputs until str0m asks us to wait.
        let timeout = loop {
            match rtc.poll_output() {
                Ok(Output::Timeout(t)) => break t,
                Ok(Output::Transmit(t)) => {
                    let _ = socket.send_to(&t.contents, t.destination).await;
                }
                Ok(Output::Event(ev)) => {
                    if let Event::ChannelOpen(id, _label) = ev {
                        if id == hid {
                            if let Some(tx) = ready_tx.take() {
                                let _ = tx.send(Ok(()));
                            }
                        }
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

        let now = Instant::now();
        let wait = timeout.saturating_duration_since(now);
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
                    None => return, // handle dropped
                }
            }
        }
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

    // Response: {"sd": "<base64 of {type:answer, sdp:...}>"}.
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
