//! Live keyboard/pointer check for the JetKVM, one step per run so the result can be
//! inspected between steps. Aimed at a Firefox address bar at the top of a 16:9 host
//! screen; it never presses Enter, so nothing is submitted.
//!
//! `JETKVM_STEP` is one of `point` (move over the address bar), `click-type` (click it
//! and type a short string), `clear` (Ctrl+A, Backspace, Escape), `caps` (tap Caps Lock; run twice to restore).
//!
//! ```bash
//! JETKVM_HOST=10.0.10.21 JETKVM_PASS='...' JETKVM_STEP=point \
//!   cargo run -p flashdk-adapters --example jetkvm_input
//! ```

use std::sync::Arc;
use std::time::Duration;

use flashdk_adapters::jetkvm::JetKvm;
use flashdk_adapters::tls_pin::MemoryPinStore;
use flashdk_core::hid::{AbsMouse, Hid, KeyCode, KeyEvent};
use flashdk_core::TlsPolicy;

const URLBAR: (u16, u16) = (16256, 1921);

async fn tap(kvm: &JetKvm, key: u8) -> Result<(), Box<dyn std::error::Error>> {
    kvm.key(KeyEvent {
        key: KeyCode(key),
        pressed: true,
    })
    .await?;
    tokio::time::sleep(Duration::from_millis(60)).await;
    kvm.key(KeyEvent {
        key: KeyCode(key),
        pressed: false,
    })
    .await?;
    tokio::time::sleep(Duration::from_millis(60)).await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("JETKVM_HOST").expect("set JETKVM_HOST");
    let pass = std::env::var("JETKVM_PASS").expect("set JETKVM_PASS");
    let step = std::env::var("JETKVM_STEP").expect("set JETKVM_STEP");
    let kvm = JetKvm::connect(
        host,
        &pass,
        TlsPolicy::RequireTls,
        Arc::new(MemoryPinStore::default()),
    )
    .await?;
    let (x, y) = URLBAR;

    match step.as_str() {
        "point" => {
            kvm.absolute_mouse(AbsMouse { x, y, buttons: 0 }).await?;
        }
        "click-type" => {
            kvm.absolute_mouse(AbsMouse { x, y, buttons: 0 }).await?;
            tokio::time::sleep(Duration::from_millis(200)).await;
            kvm.absolute_mouse(AbsMouse { x, y, buttons: 1 }).await?;
            tokio::time::sleep(Duration::from_millis(80)).await;
            kvm.absolute_mouse(AbsMouse { x, y, buttons: 0 }).await?;
            tokio::time::sleep(Duration::from_millis(300)).await;
            kvm.paste_text("flashdk-test 123").await?;
        }
        "click-newsession" => {
            // Firefox "Start New Session" button on the session-restore page.
            let (bx, by) = (18536, 21022);
            kvm.absolute_mouse(AbsMouse {
                x: bx,
                y: by,
                buttons: 0,
            })
            .await?;
            tokio::time::sleep(Duration::from_millis(300)).await;
            kvm.absolute_mouse(AbsMouse {
                x: bx,
                y: by,
                buttons: 1,
            })
            .await?;
            tokio::time::sleep(Duration::from_millis(120)).await;
            kvm.absolute_mouse(AbsMouse {
                x: bx,
                y: by,
                buttons: 0,
            })
            .await?;
        }
        "caps" => {
            tap(&kvm, 0x39).await?; // Caps Lock
        }
        "clear" => {
            kvm.key(KeyEvent {
                key: KeyCode(0xE0),
                pressed: true,
            })
            .await?; // Left Ctrl
            tap(&kvm, 0x04).await?; // A
            kvm.key(KeyEvent {
                key: KeyCode(0xE0),
                pressed: false,
            })
            .await?;
            tap(&kvm, 0x2A).await?; // Backspace
            tap(&kvm, 0x29).await?; // Escape
        }
        other => panic!("unknown JETKVM_STEP {other}"),
    }
    tokio::time::sleep(Duration::from_millis(800)).await;
    println!("step {step} sent");
    Ok(())
}
