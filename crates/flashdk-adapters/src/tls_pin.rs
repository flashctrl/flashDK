//! Trust-on-first-use (TOFU) TLS certificate pinning, shared by every TLS-pinnable
//! adapter (PiKVM today; a future Redfish/UniFi adapter can reuse this unchanged).
//!
//! Devices like PiKVM ship a self-signed certificate, so normal CA-chain validation
//! can never succeed, and the historical "fix" is `danger_accept_invalid_certs(true)`,
//! which accepts *any* certificate from *anyone*, silently. TOFU is the standard,
//! principled alternative (the same model SSH uses): remember the certificate seen on
//! first connect, and refuse to proceed silently if a *different* certificate shows up
//! later, which is the actual MITM signal an app should surface to the user.
//!
//! Built on `rustls`'s "dangerous configuration" API. The escape hatch is real (we
//! are replacing chain-of-trust validation), but full signature verification is still
//! performed via `rustls::crypto::verify_tls12/13_signature`, so a peer must actually
//! hold the private key for the pinned certificate. Only the CA-trust check is
//! replaced with the pin comparison.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};

/// A certificate's identity, as a SHA-256 fingerprint of its DER encoding.
pub type Fingerprint = [u8; 32];

/// Where pinned fingerprints are kept. The SDK ships an in-memory default
/// ([`MemoryPinStore`]); an app should supply a persistent implementation (Keychain,
/// Keystore, a file) so a pin survives restarts. That's an app-layer concern, not
/// something the SDK should dictate.
pub trait PinStore: Send + Sync {
    /// The fingerprint previously pinned for `host`, if any.
    fn get(&self, host: &str) -> Option<Fingerprint>;
    /// Remember `fingerprint` as the trusted certificate for `host`.
    fn set(&self, host: &str, fingerprint: Fingerprint);
}

/// A simple in-memory pin store. Pins reset each process, fine for a session, but an
/// app wanting persistence across restarts should provide its own [`PinStore`].
#[derive(Default)]
pub struct MemoryPinStore {
    pins: Mutex<HashMap<String, Fingerprint>>,
}

impl PinStore for MemoryPinStore {
    fn get(&self, host: &str) -> Option<Fingerprint> {
        self.pins.lock().ok()?.get(host).copied()
    }

    fn set(&self, host: &str, fingerprint: Fingerprint) {
        if let Ok(mut pins) = self.pins.lock() {
            pins.insert(host.to_string(), fingerprint);
        }
    }
}

/// A `rustls` server-certificate verifier implementing trust-on-first-use against a
/// [`PinStore`], scoped to one `host`.
#[derive(Debug)]
pub struct TofuVerifier {
    host: String,
    store: Arc<dyn PinStore>,
    provider: CryptoProvider,
}

impl std::fmt::Debug for dyn PinStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<PinStore>")
    }
}

impl TofuVerifier {
    pub fn new(
        host: impl Into<String>,
        store: Arc<dyn PinStore>,
        provider: CryptoProvider,
    ) -> Self {
        Self {
            host: host.into(),
            store,
            provider,
        }
    }
}

impl ServerCertVerifier for TofuVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let mut hasher = Sha256::new();
        hasher.update(end_entity.as_ref());
        let fingerprint: Fingerprint = hasher.finalize().into();

        match self.store.get(&self.host) {
            None => {
                // First contact: pin what we see. This is the trust-on-first-use
                // moment: a real client surfaces this to the user (e.g. "connecting
                // to <host> for the first time; certificate fingerprint: <hex>").
                self.store.set(&self.host, fingerprint);
                Ok(ServerCertVerified::assertion())
            }
            Some(pinned) if pinned == fingerprint => Ok(ServerCertVerified::assertion()),
            Some(_) => Err(rustls::Error::General(format!(
                "certificate for {} changed since it was first trusted, which is either \
                 a MITM or a re-keyed device; refusing to connect silently",
                self.host
            ))),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Build a `reqwest::Client` that speaks TLS to `host` using trust-on-first-use
/// pinning against `store`, with no built-in CA roots (a pin is the only trust
/// anchor, which is the point).
pub fn tofu_client(host: &str, store: Arc<dyn PinStore>) -> Result<reqwest::Client, String> {
    let provider = rustls::crypto::aws_lc_rs::default_provider();
    let verifier = Arc::new(TofuVerifier::new(host, store, provider.clone()));

    let config = rustls::ClientConfig::builder_with_provider(Arc::new(provider))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();

    reqwest::Client::builder()
        .use_preconfigured_tls(config)
        .build()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_roundtrip() {
        let store = MemoryPinStore::default();
        assert_eq!(store.get("h"), None);
        store.set("h", [1u8; 32]);
        assert_eq!(store.get("h"), Some([1u8; 32]));
        // A different host is unaffected.
        assert_eq!(store.get("other"), None);
    }

    /// The actual security property: first contact pins silently, a repeat of the same
    /// certificate keeps working, and a *different* certificate for the same host is
    /// rejected rather than silently accepted (the MITM/re-key signal TOFU exists for).
    #[test]
    fn tofu_pins_then_rejects_a_changed_certificate() {
        let store: Arc<dyn PinStore> = Arc::new(MemoryPinStore::default());
        let provider = rustls::crypto::aws_lc_rs::default_provider();
        let verifier = TofuVerifier::new("device.local", store, provider);

        let cert_a = CertificateDer::from(vec![1, 2, 3]);
        let cert_b = CertificateDer::from(vec![4, 5, 6]);
        let name = ServerName::try_from("device.local").unwrap();
        let now = UnixTime::now();

        assert!(verifier
            .verify_server_cert(&cert_a, &[], &name, &[], now)
            .is_ok());
        assert!(
            verifier
                .verify_server_cert(&cert_a, &[], &name, &[], now)
                .is_ok(),
            "the pinned certificate must keep being accepted"
        );
        assert!(
            verifier
                .verify_server_cert(&cert_b, &[], &name, &[], now)
                .is_err(),
            "a different certificate for an already-pinned host must be rejected"
        );
    }
}
