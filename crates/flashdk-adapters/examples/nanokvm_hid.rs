//! Live HID check for the NanoKVM: taps Caps Lock (observable through the device's LED
//! state) and moves the pointer to screen centre. Run it twice to restore Caps Lock.
//! Sends no text and never touches power.
//!
//! ```bash
//! NANOKVM_HOST=10.0.10.11 NANOKVM_TOKEN='...' FLASHDK_TLS=prefer \
//!   cargo run -p flashdk-adapters --example nanokvm_hid
//! ```

use std::sync::Arc;
use std::time::Duration;

use flashdk_adapters::nanokvm::NanoKvm;
use flashdk_adapters::tls_pin::MemoryPinStore;
use flashdk_core::hid::{AbsMouse, Hid, KeyCode, KeyEvent};
use flashdk_core::TlsPolicy;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("NANOKVM_HOST").expect("set NANOKVM_HOST");
    let token = std::env::var("NANOKVM_TOKEN").expect("set NANOKVM_TOKEN");
    let policy = match std::env::var("FLASHDK_TLS").as_deref() {
        Ok("require") => TlsPolicy::RequireTls,
        Ok("cleartext") => TlsPolicy::CleartextOnly,
        _ => TlsPolicy::PreferTls,
    };
    let kvm =
        NanoKvm::connect_with_token(host, &token, policy, Arc::new(MemoryPinStore::default()))
            .await?;

    kvm.key(KeyEvent {
        key: KeyCode(0x39),
        pressed: true,
    })
    .await?; // Caps Lock
    tokio::time::sleep(Duration::from_millis(80)).await;
    kvm.key(KeyEvent {
        key: KeyCode(0x39),
        pressed: false,
    })
    .await?;
    kvm.absolute_mouse(AbsMouse {
        x: 16384,
        y: 16384,
        buttons: 0,
    })
    .await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("sent: Caps Lock tap, pointer to centre");
    Ok(())
}
