//! Live proof-of-life for the NanoKVM adapter: opens the HID WebSocket with a session
//! token, then prints capabilities. Sends no input: zero side effects.
//!
//! Password login is quarantined (see `nanokvm/PROVENANCE.md`). Sign in through the
//! device's own web interface and copy the value of its `nano-kvm-token` cookie.
//!
//! ```bash
//! NANOKVM_HOST=10.0.10.10 NANOKVM_TOKEN='...' \
//!   cargo run -p flashdk-adapters --example nanokvm_demo
//! ```

use flashdk_adapters::nanokvm::NanoKvm;
use flashdk_core::media::VirtualMedia;
use flashdk_core::power::Power;
use flashdk_core::Device;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("NANOKVM_HOST").expect("set NANOKVM_HOST");
    let token = std::env::var("NANOKVM_TOKEN").expect("set NANOKVM_TOKEN");

    println!("Opening HID WebSocket with the supplied session token…");
    let kvm = NanoKvm::connect_with_token(host, &token).await?;
    println!(
        "Connected to {} ({:?})",
        kvm.info().model,
        kvm.info().vendor
    );
    println!("Transport: {:?}", kvm.transport_kind());
    println!("Capabilities: {:#?}", kvm.capabilities());
    match kvm.state().await {
        Ok(st) => println!("Power state: {:?}", st),
        Err(e) => println!("Power state: unavailable ({e})"),
    }
    match kvm.list().await {
        Ok(imgs) => println!("Virtual-media images: {}", imgs.len()),
        Err(e) => println!("Media list: unavailable ({e})"),
    }
    println!("WebSocket OK, no input was sent.");
    Ok(())
}
