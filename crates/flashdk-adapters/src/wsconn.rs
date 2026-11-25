//! Policy-aware connection helpers shared by the adapters that can speak either
//! HTTP/WS or HTTPS/WSS (NanoKVM, JetKVM).
//!
//! [`connect_with_policy`] runs one connection attempt per scheme the caller's
//! [`TlsPolicy`] allows, stepping from HTTPS to cleartext only when
//! [`fallback_allowed`] says nothing was listening for TLS. [`open_ws`] opens a
//! WebSocket over either scheme; for HTTPS the certificate is checked by the same
//! trust-on-first-use pin store as the REST clients (see [`crate::tls_pin`]).

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use flashdk_core::{Error, Result, TlsPolicy};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::WebSocketStream;

use crate::scheme::{fallback_allowed, schemes_to_try, Scheme};
use crate::tls_pin::{tofu_config, PinStore};

const TCP_TIMEOUT: Duration = Duration::from_secs(10);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Any byte stream a WebSocket can ride on (plain TCP or TLS over TCP).
pub trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

/// A WebSocket over either scheme.
pub type WsStream = WebSocketStream<Box<dyn Io>>;

/// One failed connection attempt, with whether a cleartext retry is permitted.
#[derive(Debug)]
pub struct Attempt {
    pub error: Error,
    pub downgrade_ok: bool,
}

impl Attempt {
    /// A failure that must end the connection (certificate, handshake, auth, ...).
    pub fn fatal(error: Error) -> Self {
        Self {
            error,
            downgrade_ok: false,
        }
    }

    /// Wrap a transport error, allowing a downgrade only if its source chain shows
    /// nothing was listening (see [`fallback_allowed`]).
    pub fn from_source(error: Error, source: &(dyn std::error::Error + 'static)) -> Self {
        Self {
            downgrade_ok: fallback_allowed(source),
            error,
        }
    }
}

/// Run `attempt` for each scheme the policy allows, returning the first success and
/// the scheme it used. A failure ends the search unless it permits a downgrade and
/// another scheme remains.
pub async fn connect_with_policy<T, F, Fut>(
    policy: TlsPolicy,
    host: &str,
    store: &Arc<dyn PinStore>,
    mut attempt: F,
) -> Result<(T, Scheme)>
where
    F: FnMut(Scheme) -> Fut,
    Fut: Future<Output = std::result::Result<T, Attempt>>,
{
    let schemes = schemes_to_try(policy, store.get(host).is_some());
    let mut last: Option<Error> = None;
    for &scheme in schemes {
        match attempt(scheme).await {
            Ok(v) => return Ok((v, scheme)),
            Err(a) => {
                let stop = !a.downgrade_ok;
                last = Some(a.error);
                if stop {
                    break;
                }
            }
        }
    }
    Err(last.unwrap_or_else(|| Error::Transport("no connection scheme allowed".into())))
}

/// `host` may carry a port ("192.0.2.10:8080"); otherwise use the scheme's default.
fn authority(host: &str, scheme: Scheme) -> String {
    let has_port = host.rsplit_once(':').is_some_and(|(h, p)| {
        !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) && !h.ends_with(':')
    });
    if has_port {
        host.to_string()
    } else {
        let port = match scheme {
            Scheme::Https => 443,
            Scheme::Http => 80,
        };
        format!("{host}:{port}")
    }
}

/// The name for TLS SNI/verification: `host` without port or IPv6 brackets.
fn tls_name(host: &str) -> String {
    let h = authority(host, Scheme::Https);
    let h = h.rsplit_once(':').map_or(h.as_str(), |(h, _)| h);
    h.trim_start_matches('[').trim_end_matches(']').to_string()
}

/// Open a WebSocket to `{ws|wss}://{host}{path}` with extra request `headers`.
/// Over TLS the server certificate is checked against `store` (TOFU).
pub async fn open_ws(
    scheme: Scheme,
    host: &str,
    path: &str,
    headers: &[(&str, String)],
    store: &Arc<dyn PinStore>,
) -> std::result::Result<WsStream, Attempt> {
    let url = match scheme {
        Scheme::Https => format!("wss://{host}{path}"),
        Scheme::Http => format!("ws://{host}{path}"),
    };
    let mut req = url
        .into_client_request()
        .map_err(|e| Attempt::fatal(Error::Transport(e.to_string())))?;
    for (k, v) in headers {
        let name = tokio_tungstenite::tungstenite::http::HeaderName::from_bytes(k.as_bytes())
            .map_err(|_| Attempt::fatal(Error::Protocol("bad header name".into())))?;
        let val = v
            .parse()
            .map_err(|_| Attempt::fatal(Error::Protocol("bad header value".into())))?;
        req.headers_mut().insert(name, val);
    }

    let tcp = match tokio::time::timeout(TCP_TIMEOUT, TcpStream::connect(authority(host, scheme)))
        .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return Err(Attempt::from_source(Error::Transport(e.to_string()), &e)),
        Err(_) => {
            let e = std::io::Error::from(std::io::ErrorKind::TimedOut);
            return Err(Attempt::from_source(
                Error::Transport("connect timed out".into()),
                &e,
            ));
        }
    };

    let io: Box<dyn Io> = match scheme {
        Scheme::Http => Box::new(tcp),
        Scheme::Https => {
            let config = tofu_config(host, store.clone())
                .map_err(|e| Attempt::fatal(Error::Transport(e)))?;
            let name = rustls::pki_types::ServerName::try_from(tls_name(host))
                .map_err(|e| Attempt::fatal(Error::Transport(e.to_string())))?;
            let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
            let tls = tokio::time::timeout(HANDSHAKE_TIMEOUT, connector.connect(name, tcp))
                .await
                .map_err(|_| Attempt::fatal(Error::Transport("TLS handshake timed out".into())))?
                .map_err(|e| Attempt::fatal(Error::Transport(format!("TLS: {e}"))))?;
            Box::new(tls)
        }
    };

    let (ws, _resp) =
        tokio::time::timeout(HANDSHAKE_TIMEOUT, tokio_tungstenite::client_async(req, io))
            .await
            .map_err(|_| Attempt::fatal(Error::Transport("websocket handshake timed out".into())))?
            .map_err(|e| {
                use tokio_tungstenite::tungstenite::Error as WsError;
                match &e {
                    WsError::Http(resp) if resp.status().is_redirection() => {
                        redirect_error(resp.headers())
                    }
                    WsError::Http(resp) if resp.status() == 401 => {
                        Attempt::fatal(Error::Auth("session rejected; sign in again".into()))
                    }
                    _ => Attempt::fatal(Error::Transport(e.to_string())),
                }
            })?;
    Ok(ws)
}

/// A cleartext request answered with a redirect: when it points at `https://` the device
/// has HTTPS enforced, which the caller can fix by choosing a TLS policy.
pub fn redirect_error(headers: &tokio_tungstenite::tungstenite::http::HeaderMap) -> Attempt {
    let to_https = headers
        .get("location")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|l| l.starts_with("https://"));
    Attempt::fatal(if to_https {
        Error::TlsRequired
    } else {
        Error::Transport("unexpected redirect from the device".into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tls_pin::MemoryPinStore;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn store() -> Arc<dyn PinStore> {
        Arc::new(MemoryPinStore::default())
    }

    #[test]
    fn authority_adds_default_ports_only_when_missing() {
        assert_eq!(authority("192.0.2.1", Scheme::Https), "192.0.2.1:443");
        assert_eq!(authority("192.0.2.1", Scheme::Http), "192.0.2.1:80");
        assert_eq!(authority("192.0.2.1:8080", Scheme::Http), "192.0.2.1:8080");
        assert_eq!(tls_name("192.0.2.1:8443"), "192.0.2.1");
        assert_eq!(tls_name("kvm.example"), "kvm.example");
    }

    #[tokio::test]
    async fn downgrades_only_when_attempt_allows_it() {
        let calls = AtomicUsize::new(0);
        let st = store();
        let r = connect_with_policy(TlsPolicy::PreferTls, "h", &st, |s| {
            calls.fetch_add(1, Ordering::SeqCst);
            async move {
                match s {
                    Scheme::Https => Err(Attempt {
                        error: Error::Transport("refused".into()),
                        downgrade_ok: true,
                    }),
                    Scheme::Http => Ok(7),
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(r, (7, Scheme::Http));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_fatal_https_error_never_reaches_cleartext() {
        let calls = AtomicUsize::new(0);
        let st = store();
        let r: Result<(u8, Scheme)> = connect_with_policy(TlsPolicy::PreferTls, "h", &st, |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Err(Attempt::fatal(Error::Transport("cert changed".into()))) }
        })
        .await;
        assert!(r.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn require_tls_makes_one_https_attempt() {
        let calls = AtomicUsize::new(0);
        let st = store();
        let r: Result<(u8, Scheme)> = connect_with_policy(TlsPolicy::RequireTls, "h", &st, |s| {
            calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(s, Scheme::Https);
            async {
                Err(Attempt {
                    error: Error::Transport("refused".into()),
                    downgrade_ok: true,
                })
            }
        })
        .await;
        assert!(r.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn https_redirect_maps_to_tls_required() {
        use tokio_tungstenite::tungstenite::http::HeaderMap;
        let mut h = HeaderMap::new();
        h.insert("location", "https://192.0.2.1/".parse().unwrap());
        assert_eq!(redirect_error(&h).error, Error::TlsRequired);
        assert!(!redirect_error(&h).downgrade_ok);
        assert!(matches!(
            redirect_error(&HeaderMap::new()).error,
            Error::Transport(_)
        ));
    }

    #[tokio::test]
    async fn refused_port_reports_a_downgradable_attempt() {
        // Port 1 on loopback has no listener: ConnectionRefused.
        let st = store();
        let err = open_ws(Scheme::Http, "127.0.0.1:1", "/x", &[], &st)
            .await
            .err()
            .expect("must fail");
        assert!(err.downgrade_ok);
    }
}

#[cfg(test)]
mod tls_tests {
    use super::*;
    use crate::tls_pin::MemoryPinStore;
    use futures_util::{SinkExt, StreamExt};
    use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
    use sha2::{Digest, Sha256};
    use tokio_tungstenite::tungstenite::Message;

    /// A loopback `wss://` echo server with a fresh self-signed certificate.
    /// Returns its `host:port` and the certificate's SHA-256 fingerprint.
    async fn tls_server() -> (String, [u8; 32]) {
        let ck = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
        let der = ck.cert.der().clone();
        let fp: [u8; 32] = Sha256::digest(der.as_ref()).into();
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(ck.signing_key.serialize_der()));
        let cfg = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![der], key)
        .unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(cfg));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((tcp, _)) = listener.accept().await {
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    let Ok(tls) = acceptor.accept(tcp).await else {
                        return;
                    };
                    let Ok(mut ws) = tokio_tungstenite::accept_async(tls).await else {
                        return;
                    };
                    while let Some(Ok(m)) = ws.next().await {
                        if m.is_text() {
                            let _ = ws.send(m).await;
                        }
                    }
                });
            }
        });
        (addr.to_string(), fp)
    }

    #[tokio::test]
    async fn first_contact_pins_then_the_same_cert_is_accepted() {
        let (host, fp) = tls_server().await;
        let store: Arc<dyn PinStore> = Arc::new(MemoryPinStore::default());
        assert!(store.get(&host).is_none());

        let mut ws = open_ws(Scheme::Https, &host, "/", &[], &store)
            .await
            .map_err(|a| a.error)
            .expect("first contact over TLS");
        assert_eq!(store.get(&host), Some(fp), "TOFU pinned the certificate");
        ws.send(Message::Text("hi".into())).await.unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap(),
            Message::Text("hi".into())
        );

        open_ws(Scheme::Https, &host, "/", &[], &store)
            .await
            .map_err(|a| a.error)
            .expect("same certificate again");
    }

    #[tokio::test]
    async fn a_changed_certificate_is_refused_and_never_downgraded() {
        let (host, _fp) = tls_server().await;
        let store: Arc<dyn PinStore> = Arc::new(MemoryPinStore::default());
        store.set(&host, [0xAA; 32]); // pinned to some other certificate

        let err = open_ws(Scheme::Https, &host, "/", &[], &store)
            .await
            .err()
            .expect("mismatch must fail");
        assert!(
            !err.downgrade_ok,
            "a handshake failure never permits cleartext"
        );

        // And through the policy layer: PreferTls with a stored pin tries HTTPS only.
        let tried = std::sync::Mutex::new(Vec::new());
        let r: Result<(WsStream, Scheme)> =
            connect_with_policy(TlsPolicy::PreferTls, &host, &store, |s| {
                tried.lock().unwrap().push(s);
                let (host, store) = (host.clone(), store.clone());
                async move { open_ws(s, &host, "/", &[], &store).await }
            })
            .await;
        assert!(r.is_err());
        assert_eq!(*tried.lock().unwrap(), vec![Scheme::Https]);
    }

    #[tokio::test]
    async fn cleartext_only_reaches_a_plain_server_without_trying_tls() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((tcp, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let _ = tokio_tungstenite::accept_async(tcp).await;
                });
            }
        });
        let host = addr.to_string();
        let store: Arc<dyn PinStore> = Arc::new(MemoryPinStore::default());
        let (_ws, scheme) = connect_with_policy(TlsPolicy::CleartextOnly, &host, &store, |s| {
            let (host, store) = (host.clone(), store.clone());
            async move { open_ws(s, &host, "/", &[], &store).await }
        })
        .await
        .map_err(|e| e.to_string())
        .expect("cleartext-only reaches a plain server");
        assert_eq!(scheme, Scheme::Http);
    }
}
