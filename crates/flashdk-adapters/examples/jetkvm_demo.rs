//! Live proof-of-life for the JetKVM adapter: log in, bring up the WebRTC connection,
//! and move the mouse to screen-center on the attached target.
//!
//! JetKVM's USB HID drives whatever it's plugged into, so run this only when JetKVM is
//! on a disposable target (not your dev host).
//!
//! ```bash
//! JETKVM_HOST=10.0.10.21 JETKVM_PASS='...' \
//!   cargo run -p flashdk-adapters --example jetkvm_demo
//! ```

use std::time::Duration;

use flashdk_adapters::jetkvm::JetKvm;
use flashdk_core::hid::{AbsMouse, Hid};
use flashdk_core::Device;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("JETKVM_HOST").expect("set JETKVM_HOST");
    let pass = std::env::var("JETKVM_PASS").expect("set JETKVM_PASS");

    println!("Connecting to JetKVM (login + WebRTC handshake)…");
    let kvm = tokio::time::timeout(Duration::from_secs(25), JetKvm::connect(host, &pass))
        .await
        .map_err(|_| "connect timed out: WebRTC/hidrpc channel never opened")??;

    println!(
        "Connected: {} ({:?}), transport {:?}",
        kvm.info().model,
        kvm.info().vendor,
        kvm.transport_kind()
    );

    match kvm.local_version().await {
        Ok(v) => println!("rpc getLocalVersion -> {v}"),
        Err(e) => println!("rpc getLocalVersion failed: {e}"),
    }

    println!("Moving mouse to center (16384,16384) on the target…");
    kvm.absolute_mouse(AbsMouse {
        x: 16384,
        y: 16384,
        buttons: 0,
    })
    .await?;
    // Give the driver a moment to flush the datagram before we exit.
    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("Sent. If a display is attached to the target, its cursor jumped to center.");
    Ok(())
}
