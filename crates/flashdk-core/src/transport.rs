//! The single most important architectural fact in this SDK.
//!
//! Our devices expose control in two structurally different ways. This isn't a
//! detail: it dictates how the whole session is set up, so we name it explicitly.

/// The two shapes of "how do I send this device a command?"
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    /// **Request/response.** PiKVM and NanoKVM: you make an HTTP call and get an
    /// answer, with a WebSocket alongside for events and low-latency input. Sending a
    /// keystroke is a self-contained request, no long-lived peer needed.
    RequestResponse,

    /// **Peer RPC.** JetKVM: control *is* JSON-RPC carried over a WebRTC DataChannel.
    /// You cannot send a single keystroke until a full WebRTC peer connection is
    /// negotiated and the DataChannel is open. The peer connection is not just for
    /// video here: it's the command bus itself.
    PeerRpc,
}

impl TransportKind {
    /// Does this transport require a WebRTC peer connection before *any* control is
    /// possible? The app uses this to decide how much to spin up on "Connect".
    pub fn requires_peer_connection(self) -> bool {
        matches!(self, TransportKind::PeerRpc)
    }
}

/// What an app is willing to accept when it connects. Defaults to the strictest.
///
/// Most KVMs ship with HTTPS switched off, so a strict default means an app must make
/// an explicit, visible choice before sending credentials in cleartext.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TlsPolicy {
    /// HTTPS only. Never falls back to cleartext. The default.
    #[default]
    RequireTls,

    /// Try HTTPS first and fall back to cleartext only when nothing is listening for
    /// TLS. It never falls back after a certificate problem, and never for a device
    /// previously seen over TLS (a stored pin means TLS is expected), because an
    /// attacker who can block the HTTPS port could otherwise force a downgrade.
    PreferTls,

    /// Cleartext only, skipping any TLS attempt. For devices the user knows have TLS
    /// off; an explicit opt-in.
    CleartextOnly,
}

/// How a live connection is secured. Reported by the device after connecting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConnectionSecurity {
    /// Encrypted. `pinned` is the SHA-256 fingerprint of the certificate trusted for
    /// this device, if one has been recorded.
    Tls { pinned: Option<[u8; 32]> },

    /// Credentials and input travel unencrypted. Apps should warn.
    Cleartext,
}

impl ConnectionSecurity {
    /// Is the connection encrypted?
    pub fn is_encrypted(self) -> bool {
        matches!(self, ConnectionSecurity::Tls { .. })
    }
}
