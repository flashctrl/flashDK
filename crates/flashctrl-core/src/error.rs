//! One error type for the whole SDK.
//!
//! Rust functions that can fail return `Result<T, Error>`. We alias that to
//! [`Result<T>`] so signatures stay short. Every variant below is something a *caller*
//! (the app) might want to react to differently — e.g. show a re-login prompt on
//! [`Error::Auth`], or grey out a button on [`Error::NotSupported`].

use std::fmt;

/// Shorthand for `std::result::Result<T, Error>` used throughout the SDK.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong talking to a KVM, in vendor-neutral terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The device genuinely lacks this capability (e.g. NanoKVM has no ATX *reset*).
    /// The app should hide or disable the feature, not retry.
    NotSupported(&'static str),
    /// We haven't written this code path yet. Every adapter stub returns this today.
    NotImplemented,
    /// Login/session problem — credentials rejected, token expired, etc.
    Auth(String),
    /// Network/transport failure — unreachable host, TLS problem, dropped socket.
    Transport(String),
    /// The device answered, but not in a shape we understood.
    Protocol(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotSupported(what) => write!(f, "capability not supported: {what}"),
            Error::NotImplemented => write!(f, "not implemented yet"),
            Error::Auth(msg) => write!(f, "authentication error: {msg}"),
            Error::Transport(msg) => write!(f, "transport error: {msg}"),
            Error::Protocol(msg) => write!(f, "protocol error: {msg}"),
        }
    }
}

// Implementing the standard Error trait lets our error interoperate with the wider
// Rust ecosystem (the `?` operator, error-reporting libraries, etc.).
impl std::error::Error for Error {}
