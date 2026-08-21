//! Live proof-of-life for the PiKVM HID adapter.
//!
//! Reads credentials from the environment so nothing sensitive lives in the repo:
//!
//! ```bash
//! PIKVM_HOST=10.0.10.20 PIKVM_USER=admin PIKVM_PASS='...' \
//!   cargo run -p flashdk-adapters --example pikvm_demo
//! ```
//!
//! It connects, prints what the device can do, then moves the mouse to screen-center
//! — a deliberately harmless action. (It will move the real cursor on whatever is
//! attached to the PiKVM.)

use flashdk_adapters::pikvm::PiKvm;
use flashdk_core::hid::{AbsMouse, Hid};
use flashdk_core::Device;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("PIKVM_HOST").expect("set PIKVM_HOST");
    let user = std::env::var("PIKVM_USER").expect("set PIKVM_USER");
    let pass = std::env::var("PIKVM_PASS").expect("set PIKVM_PASS");

    let kvm = PiKvm::new(host, user, pass)?;

    let info = kvm.info();
    println!("Connected: {} ({:?})", info.model, info.vendor);
    println!("Transport: {:?}", kvm.transport_kind());
    println!("Capabilities: {:#?}", kvm.capabilities());

    // Move the pointer to the center of the screen (16384 ≈ midpoint of 0..=32767).
    println!("Moving mouse to center…");
    kvm.absolute_mouse(AbsMouse {
        x: 16384,
        y: 16384,
        buttons: 0,
    })
    .await?;
    println!("Done. If a display is attached, the cursor jumped to center.");

    Ok(())
}
