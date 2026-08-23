//! JetKVM WebRTC transport (sans-IO via str0m) — foundation.
//!
//! JetKVM speaks [`TransportKind::PeerRpc`](flashdk_core::TransportKind): control only
//! works over a WebRTC peer connection. The connection lifecycle:
//!
//! 1. Build a [`str0m::Rtc`] as the offerer and add the data channels the device
//!    expects (`rpc` for JSON-RPC control; `hidrpc` + `hidrpc-unreliable-*` for the
//!    binary HID frames in [`super::wire`]).
//! 2. Generate an SDP offer and exchange it with the device via [`exchange_sdp`]
//!    (`POST /webrtc/session`), receiving the answer.
//! 3. Drive str0m's sans-IO loop over a UDP socket: pump `poll_output()` →
//!    transmit/await-timeout, feed inbound datagrams via `handle_input()`, and
//!    dispatch channel-open / channel-data events.
//! 4. Once the `hidrpc` channel opens, write the [`super::wire`] frames.
//!
//! This module currently implements the signaling exchange and the offer/channel
//! setup; the sans-IO driver loop (step 3) and HID wiring land next.

#![allow(dead_code)] // built ahead of the adapter that will drive it

use flashdk_core::{Error, Result};

/// The data-channel labels JetKVM uses (observed on the wire).
pub const CH_RPC: &str = "rpc";
pub const CH_HID: &str = "hidrpc";
pub const CH_HID_UNRELIABLE_ORDERED: &str = "hidrpc-unreliable-ordered";

/// Exchange an SDP offer with the device and return its SDP answer.
///
/// JetKVM's `POST /webrtc/session` expects the body to be a JSON string whose value is
/// the base64 of the offer, and replies in kind (established by probing — see
/// docs/captures/jetkvm-datachannel-hid.md). The exact inner encoding is confirmed by
/// iterating a real str0m offer against the live device, so this is intentionally the
/// single place that needs live verification.
pub async fn exchange_sdp(
    http: &reqwest::Client,
    base_url: &str,
    offer_sdp: &str,
) -> Result<String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(offer_sdp);
    // Body is a JSON string (quoted) whose value is the base64 offer.
    let body = serde_json::to_string(&b64).map_err(|e| Error::Protocol(e.to_string()))?;
    let resp = http
        .post(format!("{base_url}/webrtc/session"))
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| Error::Transport(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(Error::Protocol(format!(
            "signaling failed: HTTP {}",
            resp.status()
        )));
    }
    // Response is a JSON string of base64(answer SDP).
    let answer_b64: String = resp
        .json()
        .await
        .map_err(|e| Error::Protocol(e.to_string()))?;
    let answer_bytes = base64::engine::general_purpose::STANDARD
        .decode(answer_b64.trim())
        .map_err(|e| Error::Protocol(format!("answer not base64: {e}")))?;
    String::from_utf8(answer_bytes).map_err(|e| Error::Protocol(e.to_string()))
}

/// Build the offerer `Rtc` with JetKVM's data channels and produce the SDP offer to
/// send via [`exchange_sdp`]. Returns the `Rtc`, the pending offer handle, and the
/// channel id for `hidrpc` (where HID frames are written once open).
pub fn build_offer() -> Result<PendingConnection> {
    let mut rtc = str0m::Rtc::new();
    let mut api = rtc.sdp_api();
    // Order/labels mirror the observed client.
    let _rpc = api.add_channel(CH_RPC.to_string());
    let hid = api.add_channel(CH_HID.to_string());
    let _hid_unreliable = api.add_channel(CH_HID_UNRELIABLE_ORDERED.to_string());
    let (offer, pending) = api
        .apply()
        .ok_or_else(|| Error::Protocol("no offer produced".into()))?;
    Ok(PendingConnection {
        rtc,
        offer_sdp: offer.to_sdp_string(),
        pending,
        hid_channel: hid,
    })
}

/// An offer that has been created but not yet negotiated. Hand `offer_sdp` to
/// [`exchange_sdp`], then accept the answer to finish setup (next increment).
pub struct PendingConnection {
    pub rtc: str0m::Rtc,
    pub offer_sdp: String,
    pub pending: str0m::change::SdpPendingOffer,
    pub hid_channel: str0m::channel::ChannelId,
}
