//! The single most important architectural fact in this SDK.
//!
//! Our devices expose control in two structurally different ways. This isn't a
//! detail — it dictates how the whole session is set up, so we name it explicitly.

/// The two shapes of "how do I send this device a command?"
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    /// **Request/response.** PiKVM and NanoKVM: you make an HTTP call and get an
    /// answer, with a WebSocket alongside for events and low-latency input. Sending a
    /// keystroke is a self-contained request — no long-lived peer needed.
    RequestResponse,

    /// **Peer RPC.** JetKVM: control *is* JSON-RPC carried over a WebRTC DataChannel.
    /// You cannot send a single keystroke until a full WebRTC peer connection is
    /// negotiated and the DataChannel is open. The peer connection is not just for
    /// video here — it's the command bus itself.
    PeerRpc,
}

impl TransportKind {
    /// Does this transport require a WebRTC peer connection before *any* control is
    /// possible? The app uses this to decide how much to spin up on "Connect".
    pub fn requires_peer_connection(self) -> bool {
        matches!(self, TransportKind::PeerRpc)
    }
}
