//! One error type for the whole SDK.
//!
//! Rust functions that can fail return `Result<T, Error>`. We alias that to
//! [`Result<T>`] so signatures stay short. Every variant below is something a *caller*
//! (the app) might want to react to differently, e.g. show a re-login prompt on
//! [`Error::Auth`], or grey out a button on [`Error::NotSupported`].

use std::fmt;

/// Shorthand for `std::result::Result<T, Error>` used throughout the SDK.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong talking to a KVM, in vendor-neutral terms.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The device genuinely lacks this capability (e.g. NanoKVM has no ATX *reset*).
    /// The app should hide or disable the feature, not retry.
    NotSupported(&'static str),
    /// We haven't written this code path yet. Every adapter stub returns this today.
    NotImplemented,
    /// Login/session problem: credentials rejected, token expired, etc.
    Auth(String),
    /// Network/transport failure: unreachable host, TLS problem, dropped socket.
    Transport(String),
    /// The device answered, but not in a shape we understood.
    Protocol(String),
    /// The device only accepts HTTPS (it redirected plain HTTP to it), but the caller
    /// asked for cleartext. Retry with `TlsPolicy::RequireTls` or `PreferTls`.
    TlsRequired,
    /// The device presented a different TLS certificate from the one pinned for `host`.
    /// Either it was re-keyed (a reinstall or reset) or someone is intercepting the
    /// connection. Never retry silently: tell the user, and replace the pin only if
    /// they accept.
    CertificateChanged {
        host: String,
        /// SHA-256 of the certificate the device presented this time. Show it to the
        /// user. On acceptance, store exactly this value as the pin for `host` and
        /// reconnect, so that only that certificate is trusted. Do not simply delete
        /// the old pin: that trusts whatever answers the next attempt.
        presented: [u8; 32],
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotSupported(what) => write!(f, "capability not supported: {what}"),
            Error::NotImplemented => write!(f, "not implemented yet"),
            Error::Auth(msg) => write!(f, "authentication error: {msg}"),
            Error::Transport(msg) => write!(f, "transport error: {msg}"),
            Error::Protocol(msg) => write!(f, "protocol error: {msg}"),
            Error::TlsRequired => write!(f, "the device requires HTTPS"),
            Error::CertificateChanged { host, .. } => {
                write!(
                    f,
                    "the TLS certificate for {host} changed since it was pinned"
                )
            }
        }
    }
}

// Implementing the standard Error trait lets our error interoperate with the wider
// Rust ecosystem (the `?` operator, error-reporting libraries, etc.).
impl std::error::Error for Error {}
