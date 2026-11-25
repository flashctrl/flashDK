//! UniFFI surface over the SDK, for the Android and iOS apps.
//!
//! The core traits use native `async fn`, which isn't dyn-compatible, so the four
//! adapters are held in an enum and each call is dispatched with a macro. Foreign
//! (Kotlin/Swift) types mirror the core ones and convert at the boundary, which keeps
//! `flashdk-core` free of any UniFFI dependency.

use std::fmt;
use std::sync::Arc;

use flashdk_adapters::glinet::GlInetKvm;
use flashdk_adapters::jetkvm::JetKvm;
use flashdk_adapters::nanokvm::NanoKvm;
use flashdk_adapters::pikvm::PiKvm;
use flashdk_adapters::tls_pin::{Fingerprint, MemoryPinStore, PinStore as CorePinStore};
use flashdk_core::hid::{AbsMouse, Hid, KeyCode, KeyEvent, RelMouse, Wheel};
use flashdk_core::media::VirtualMedia;
use flashdk_core::power::Power;
use flashdk_core::Device;

uniffi::setup_scaffolding!();

// --- Errors ---

/// Mirrors `flashdk_core::Error`. Fields are named `detail`, not `message`: the generated
/// Kotlin exceptions already inherit a `message` property and would not compile.
/// Details never contain credentials: the adapters
/// only put device-side text and our own descriptions in them.
#[derive(Debug, uniffi::Error)]
pub enum FlashError {
    NotSupported {
        what: String,
    },
    NotImplemented,
    Auth {
        detail: String,
    },
    Transport {
        detail: String,
    },
    Protocol {
        detail: String,
    },
    /// Raised by this layer for malformed arguments (for example a pin that isn't 32 bytes).
    InvalidArgument {
        detail: String,
    },
    /// The device only accepts HTTPS but a cleartext policy was requested. Offer the user
    /// `RequireTls` or `PreferTls`.
    TlsRequired,
    /// The device's certificate differs from the pinned one. Warn the user and show
    /// `presented` (the 32-byte SHA-256 of the new certificate). Only if they accept,
    /// call `PinStore.set(host, presented)` and connect again, so that only that exact
    /// certificate is trusted. Do not just remove the old pin.
    CertificateChanged {
        host: String,
        presented: Vec<u8>,
    },
}

impl fmt::Display for FlashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupported { what } => write!(f, "capability not supported: {what}"),
            Self::NotImplemented => write!(f, "not implemented yet"),
            Self::Auth { detail } => write!(f, "authentication error: {detail}"),
            Self::Transport { detail } => write!(f, "transport error: {detail}"),
            Self::Protocol { detail } => write!(f, "protocol error: {detail}"),
            Self::InvalidArgument { detail } => write!(f, "invalid argument: {detail}"),
            Self::TlsRequired => write!(f, "the device requires HTTPS"),
            Self::CertificateChanged { host, .. } => {
                write!(
                    f,
                    "the TLS certificate for {host} changed since it was pinned"
                )
            }
        }
    }
}

impl std::error::Error for FlashError {}

impl From<flashdk_core::Error> for FlashError {
    fn from(e: flashdk_core::Error) -> Self {
        use flashdk_core::Error as E;
        match e {
            E::NotSupported(what) => Self::NotSupported { what: what.into() },
            E::NotImplemented => Self::NotImplemented,
            E::Auth(detail) => Self::Auth { detail },
            E::Transport(detail) => Self::Transport { detail },
            E::Protocol(detail) => Self::Protocol { detail },
            E::TlsRequired => Self::TlsRequired,
            E::CertificateChanged { host, presented } => Self::CertificateChanged {
                host,
                presented: presented.to_vec(),
            },
            // `Error` is non-exhaustive: report variants added later as a protocol error.
            other => Self::Protocol {
                detail: other.to_string(),
            },
        }
    }
}

type Result<T> = std::result::Result<T, FlashError>;

// --- Mirrored value types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Vendor {
    NanoKvm,
    PiKvm,
    JetKvm,
    GlInet,
}

impl From<flashdk_core::Vendor> for Vendor {
    fn from(v: flashdk_core::Vendor) -> Self {
        match v {
            flashdk_core::Vendor::NanoKvm => Self::NanoKvm,
            flashdk_core::Vendor::PiKvm => Self::PiKvm,
            flashdk_core::Vendor::JetKvm => Self::JetKvm,
            flashdk_core::Vendor::GlInet => Self::GlInet,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DeviceInfo {
    pub vendor: Vendor,
    pub model: String,
    pub firmware: String,
    pub hardened: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Capabilities {
    pub keyboard: bool,
    pub absolute_mouse: bool,
    pub relative_mouse: bool,
    pub video_mjpeg: bool,
    pub video_h264: bool,
    pub video_webrtc: bool,
    pub power_on_off: bool,
    pub power_reset: bool,
    pub virtual_media: bool,
    pub wake_on_lan: bool,
    pub tls_pinnable: bool,
}

impl From<flashdk_core::Capabilities> for Capabilities {
    fn from(c: flashdk_core::Capabilities) -> Self {
        Self {
            keyboard: c.keyboard,
            absolute_mouse: c.absolute_mouse,
            relative_mouse: c.relative_mouse,
            video_mjpeg: c.video_mjpeg,
            video_h264: c.video_h264,
            video_webrtc: c.video_webrtc,
            power_on_off: c.power_on_off,
            power_reset: c.power_reset,
            virtual_media: c.virtual_media,
            wake_on_lan: c.wake_on_lan,
            tls_pinnable: c.tls_pinnable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PowerAction {
    On,
    ShortPress,
    LongPress,
    Reset,
}

impl From<PowerAction> for flashdk_core::power::PowerAction {
    fn from(a: PowerAction) -> Self {
        match a {
            PowerAction::On => Self::On,
            PowerAction::ShortPress => Self::ShortPress,
            PowerAction::LongPress => Self::LongPress,
            PowerAction::Reset => Self::Reset,
        }
    }
}

/// `None` means the device can't report that signal (distinct from "off").
#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct PowerState {
    pub powered: Option<bool>,
    pub hdd_activity: Option<bool>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct MediaImage {
    pub name: String,
    pub size: Option<u64>,
    pub mounted: bool,
}

/// Whether a connection may use HTTPS, cleartext, or both. `RequireTls` is the safe
/// default: use it unless the user knowingly chose otherwise. Most KVMs ship with
/// HTTPS off, so a stock device needs `PreferTls` or `CleartextOnly`.
///
/// `PreferTls` tries HTTPS first and uses cleartext only when nothing answered on the
/// HTTPS port. It never falls back after a certificate or handshake failure, nor for a
/// device whose certificate is already pinned. PiKVM and GL.iNet are HTTPS-only:
/// `CleartextOnly` is rejected for them with `NotSupported`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TlsPolicy {
    RequireTls,
    PreferTls,
    CleartextOnly,
}

impl From<TlsPolicy> for flashdk_core::TlsPolicy {
    fn from(p: TlsPolicy) -> Self {
        match p {
            TlsPolicy::RequireTls => Self::RequireTls,
            TlsPolicy::PreferTls => Self::PreferTls,
            TlsPolicy::CleartextOnly => Self::CleartextOnly,
        }
    }
}

/// How the live connection is secured. Show a warning for `Cleartext`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ConnectionSecurity {
    /// TLS; `pinned` is the SHA-256 fingerprint of the pinned certificate (32 bytes),
    /// if any. Persist it through your `PinStore` and show it for verification.
    Tls {
        pinned: Option<Vec<u8>>,
    },
    Cleartext,
}

impl From<flashdk_core::ConnectionSecurity> for ConnectionSecurity {
    fn from(s: flashdk_core::ConnectionSecurity) -> Self {
        match s {
            flashdk_core::ConnectionSecurity::Tls { pinned } => Self::Tls {
                pinned: pinned.map(|p| p.to_vec()),
            },
            // Non-exhaustive upstream: anything unknown is reported as the weaker case.
            _ => Self::Cleartext,
        }
    }
}

// --- TLS pin persistence, supplied by the app (Keystore / Keychain) ---

/// Implemented in Kotlin/Swift. Fingerprints are 32-byte SHA-256 values.
#[uniffi::export(with_foreign)]
pub trait PinStore: Send + Sync {
    fn get(&self, host: String) -> Option<Vec<u8>>;
    fn set(&self, host: String, fingerprint: Vec<u8>);
}

struct ForeignPins(Arc<dyn PinStore>);

impl CorePinStore for ForeignPins {
    fn get(&self, host: &str) -> Option<Fingerprint> {
        // A stored value of the wrong length is treated as "no pin"; the caller then
        // re-pins, so a corrupt store can't wedge a device or silently accept a cert.
        self.0.get(host.to_string())?.try_into().ok()
    }

    fn set(&self, host: &str, fingerprint: Fingerprint) {
        self.0.set(host.to_string(), fingerprint.to_vec());
    }
}

// --- The device handle ---

enum Inner {
    Nano(NanoKvm),
    Pi(PiKvm),
    Jet(JetKvm),
    Gl(GlInetKvm),
}

/// Run `$body` against whichever adapter is inside, binding it to `$d`.
macro_rules! dispatch {
    ($self:expr, $d:ident => $body:expr) => {
        match &$self.0 {
            Inner::Nano($d) => $body,
            Inner::Pi($d) => $body,
            Inner::Jet($d) => $body,
            Inner::Gl($d) => $body,
        }
    };
}

/// A connected KVM. Cheap to share; all methods are safe to call concurrently.
#[derive(uniffi::Object)]
pub struct Kvm(Inner);

fn core_store(pin_store: Option<Arc<dyn PinStore>>) -> Arc<dyn CorePinStore> {
    match pin_store {
        Some(s) => Arc::new(ForeignPins(s)),
        None => Arc::new(MemoryPinStore::default()),
    }
}

/// Connect and authenticate with a username and password. `username` is ignored by
/// JetKVM, which has only a password. `pin_store` persists TLS pins; pass `None` to
/// pin in memory for this session only. Persist pins for real use, or every launch
/// trusts whatever certificate it sees first.
///
/// NanoKVM password login is unavailable and returns `NotSupported`: use
/// [`connect_nanokvm_with_token`].
/// Upper bound for a whole connect (login, TLS, WebRTC handshake). Without it an
/// unresponsive device could leave the app waiting forever.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Run `fut`, failing with a transport error if it takes longer than `limit`.
async fn within<T>(
    limit: std::time::Duration,
    fut: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    match tokio::time::timeout(limit, fut).await {
        Ok(r) => r,
        Err(_) => Err(FlashError::Transport {
            detail: format!("connect timed out after {}s", limit.as_secs()),
        }),
    }
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn connect(
    vendor: Vendor,
    host: String,
    username: String,
    password: String,
    tls_policy: TlsPolicy,
    pin_store: Option<Arc<dyn PinStore>>,
) -> Result<Arc<Kvm>> {
    within(
        CONNECT_TIMEOUT,
        connect_inner(vendor, host, username, password, tls_policy, pin_store),
    )
    .await
}

async fn connect_inner(
    vendor: Vendor,
    host: String,
    username: String,
    password: String,
    tls_policy: TlsPolicy,
    pin_store: Option<Arc<dyn PinStore>>,
) -> Result<Arc<Kvm>> {
    let policy: flashdk_core::TlsPolicy = tls_policy.into();
    let store = core_store(pin_store);
    let inner = match vendor {
        Vendor::NanoKvm => Inner::Nano(NanoKvm::connect(host, &username, &password).await?),
        Vendor::JetKvm => Inner::Jet(JetKvm::connect(host, &password, policy, store).await?),
        Vendor::PiKvm => {
            https_only(tls_policy, "PiKVM")?;
            let kvm = PiKvm::with_pin_store(host, username, password, store)?;
            // The constructor makes no network call; this is the first authenticated
            // request, so bad credentials or an unreachable host surface here.
            kvm.refresh_identity().await?;
            Inner::Pi(kvm)
        }
        Vendor::GlInet => {
            https_only(tls_policy, "GL.iNet")?;
            Inner::Gl(GlInetKvm::connect_with_pin_store(&host, &username, &password, store).await?)
        }
    };
    Ok(Arc::new(Kvm(inner)))
}

/// Connect to a NanoKVM with the session token from its own web sign-in (the value of
/// the `nano-kvm-token` cookie). Tokens expire: on `Auth`, ask the user to sign in again.
#[uniffi::export(async_runtime = "tokio")]
pub async fn connect_nanokvm_with_token(
    host: String,
    token: String,
    tls_policy: TlsPolicy,
    pin_store: Option<Arc<dyn PinStore>>,
) -> Result<Arc<Kvm>> {
    let kvm =
        NanoKvm::connect_with_token(host, &token, tls_policy.into(), core_store(pin_store)).await?;
    Ok(Arc::new(Kvm(Inner::Nano(kvm))))
}

/// PiKVM and GL.iNet speak HTTPS only; refuse a request that demands cleartext.
fn https_only(policy: TlsPolicy, vendor: &'static str) -> Result<()> {
    if policy == TlsPolicy::CleartextOnly {
        return Err(FlashError::NotSupported {
            what: format!("{vendor} is HTTPS-only; CleartextOnly is not available"),
        });
    }
    Ok(())
}

#[uniffi::export(async_runtime = "tokio")]
impl Kvm {
    pub fn info(&self) -> DeviceInfo {
        let i = dispatch!(self, d => d.info());
        DeviceInfo {
            vendor: i.vendor.into(),
            model: i.model,
            firmware: i.firmware,
            hardened: i.hardened,
        }
    }

    pub fn capabilities(&self) -> Capabilities {
        dispatch!(self, d => d.capabilities()).into()
    }

    /// How this connection is actually secured; warn the user on `Cleartext`.
    pub fn security(&self) -> ConnectionSecurity {
        dispatch!(self, d => d.security()).into()
    }

    // Power

    pub async fn power_action(&self, action: PowerAction) -> Result<()> {
        Ok(dispatch!(self, d => d.action(action.into()).await)?)
    }

    pub async fn power_state(&self) -> Result<PowerState> {
        let s = dispatch!(self, d => d.state().await)?;
        Ok(PowerState {
            powered: s.powered,
            hdd_activity: s.hdd_activity,
        })
    }

    // Virtual media

    pub async fn media_list(&self) -> Result<Vec<MediaImage>> {
        let images = dispatch!(self, d => VirtualMedia::list(d).await)?;
        Ok(images
            .into_iter()
            .map(|m| MediaImage {
                name: m.name,
                size: m.size,
                mounted: m.mounted,
            })
            .collect())
    }

    pub async fn media_mount(&self, name: String) -> Result<()> {
        Ok(dispatch!(self, d => d.mount(&name).await)?)
    }

    pub async fn media_unmount(&self) -> Result<()> {
        Ok(dispatch!(self, d => d.unmount().await)?)
    }

    // Keyboard and mouse. `key` is a USB HID usage code (0xE1 = left shift).

    pub async fn key(&self, key: u8, pressed: bool) -> Result<()> {
        let event = KeyEvent {
            key: KeyCode(key),
            pressed,
        };
        Ok(dispatch!(self, d => d.key(event).await)?)
    }

    /// `x`/`y` are 0..=32767 on each axis; `buttons` is a bitmask (1 left, 2 right, 4 middle).
    pub async fn absolute_mouse(&self, x: u16, y: u16, buttons: u8) -> Result<()> {
        let m = AbsMouse { x, y, buttons };
        Ok(dispatch!(self, d => d.absolute_mouse(m).await)?)
    }

    pub async fn relative_mouse(&self, dx: i16, dy: i16, buttons: u8) -> Result<()> {
        let m = RelMouse { dx, dy, buttons };
        Ok(dispatch!(self, d => d.relative_mouse(m).await)?)
    }

    pub async fn wheel(&self, delta: i8) -> Result<()> {
        let w = Wheel { delta };
        Ok(dispatch!(self, d => d.wheel(w).await)?)
    }

    /// Type `text` as key events (US layout; unreachable characters are skipped).
    pub async fn paste_text(&self, text: String) -> Result<()> {
        Ok(dispatch!(self, d => d.paste_text(&text).await)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_only_vendors_reject_cleartext_only() {
        assert!(matches!(
            https_only(TlsPolicy::CleartextOnly, "PiKVM"),
            Err(FlashError::NotSupported { .. })
        ));
        assert!(https_only(TlsPolicy::RequireTls, "PiKVM").is_ok());
        assert!(https_only(TlsPolicy::PreferTls, "PiKVM").is_ok());
    }

    #[test]
    fn policy_and_security_cross_the_boundary_faithfully() {
        assert_eq!(
            flashdk_core::TlsPolicy::from(TlsPolicy::PreferTls),
            flashdk_core::TlsPolicy::PreferTls
        );
        let pinned = ConnectionSecurity::from(flashdk_core::ConnectionSecurity::Tls {
            pinned: Some([7; 32]),
        });
        assert_eq!(
            pinned,
            ConnectionSecurity::Tls {
                pinned: Some(vec![7; 32])
            }
        );
        assert_eq!(
            ConnectionSecurity::from(flashdk_core::ConnectionSecurity::Cleartext),
            ConnectionSecurity::Cleartext
        );
    }

    #[test]
    fn a_stalled_connect_times_out_with_a_transport_error() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let r: Result<()> = rt.block_on(within(
            std::time::Duration::from_millis(20),
            std::future::pending(),
        ));
        assert!(matches!(r, Err(FlashError::Transport { .. })));
    }

    #[test]
    fn a_changed_certificate_carries_the_presented_fingerprint() {
        let e = FlashError::from(flashdk_core::Error::CertificateChanged {
            host: "kvm".into(),
            presented: [9; 32],
        });
        assert!(matches!(
            e,
            FlashError::CertificateChanged { host, presented }
                if host == "kvm" && presented == vec![9u8; 32]
        ));
    }

    #[test]
    fn tls_required_maps_to_its_own_error() {
        assert!(matches!(
            FlashError::from(flashdk_core::Error::TlsRequired),
            FlashError::TlsRequired
        ));
    }
}
