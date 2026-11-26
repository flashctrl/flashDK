//! Choosing between HTTPS and HTTP for a device, per the caller's [`TlsPolicy`].
//!
//! Kept free of networking so the rules can be unit-tested exhaustively; adapters call
//! [`schemes_to_try`] for the order to attempt, and [`fallback_allowed`] to decide
//! whether a failed HTTPS attempt may be followed by cleartext.

use flashdk_core::TlsPolicy;

/// A URL scheme an adapter may connect with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Https,
    Http,
}

/// The schemes to attempt, in order. `has_pin` says whether a certificate pin is
/// already stored for this host: a stored pin means the device has been seen over TLS,
/// so `PreferTls` must not quietly step down to cleartext for it.
pub fn schemes_to_try(policy: TlsPolicy, has_pin: bool) -> &'static [Scheme] {
    match policy {
        TlsPolicy::RequireTls => &[Scheme::Https],
        TlsPolicy::PreferTls if has_pin => &[Scheme::Https],
        TlsPolicy::PreferTls => &[Scheme::Https, Scheme::Http],
        TlsPolicy::CleartextOnly => &[Scheme::Http],
        // The enum is non-exhaustive: a policy added later is treated as the strictest.
        _ => &[Scheme::Https],
    }
}

/// May a failed HTTPS attempt be followed by cleartext? Only when nothing was listening
/// for TLS: the connection was refused, timed out, or the network was unreachable.
/// A certificate mismatch, a failed handshake, or any other error must NOT downgrade,
/// since those are exactly the signs of someone interfering.
pub fn fallback_allowed(err: &(dyn std::error::Error + 'static)) -> bool {
    use std::io::ErrorKind::{ConnectionRefused, HostUnreachable, NetworkUnreachable, TimedOut};
    let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(e) = cur {
        if let Some(io) = e.downcast_ref::<std::io::Error>() {
            return matches!(
                io.kind(),
                ConnectionRefused | TimedOut | HostUnreachable | NetworkUnreachable
            );
        }
        cur = e.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;
    use std::io::{Error as IoError, ErrorKind};

    #[test]
    fn require_tls_never_offers_cleartext() {
        for pin in [false, true] {
            assert_eq!(schemes_to_try(TlsPolicy::RequireTls, pin), &[Scheme::Https]);
        }
    }

    #[test]
    fn prefer_tls_falls_back_only_without_a_pin() {
        assert_eq!(
            schemes_to_try(TlsPolicy::PreferTls, false),
            &[Scheme::Https, Scheme::Http]
        );
        assert_eq!(
            schemes_to_try(TlsPolicy::PreferTls, true),
            &[Scheme::Https],
            "a pinned device was seen over TLS, so never step down"
        );
    }

    #[test]
    fn cleartext_only_skips_tls() {
        assert_eq!(
            schemes_to_try(TlsPolicy::CleartextOnly, false),
            &[Scheme::Http]
        );
    }

    /// An error that wraps another, like reqwest's does.
    #[derive(Debug)]
    struct Wrapper(Box<dyn std::error::Error + 'static>);
    impl fmt::Display for Wrapper {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "wrapper")
        }
    }
    impl std::error::Error for Wrapper {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(self.0.as_ref())
        }
    }

    #[test]
    fn fallback_only_when_nothing_listens() {
        for kind in [
            ErrorKind::ConnectionRefused,
            ErrorKind::TimedOut,
            ErrorKind::HostUnreachable,
            ErrorKind::NetworkUnreachable,
        ] {
            let err = Wrapper(Box::new(IoError::from(kind)));
            assert!(fallback_allowed(&err), "{kind:?} should allow fallback");
        }
    }

    #[test]
    fn no_fallback_for_handshake_or_certificate_trouble() {
        // rustls surfaces certificate and handshake failures as InvalidData-style io errors.
        for kind in [
            ErrorKind::InvalidData,
            ErrorKind::ConnectionReset,
            ErrorKind::UnexpectedEof,
            ErrorKind::PermissionDenied,
        ] {
            let err = Wrapper(Box::new(IoError::from(kind)));
            assert!(!fallback_allowed(&err), "{kind:?} must not downgrade");
        }
        let no_io = Wrapper(Box::new(Wrapper(Box::new(fmt::Error))));
        assert!(!fallback_allowed(&no_io));
    }
}
