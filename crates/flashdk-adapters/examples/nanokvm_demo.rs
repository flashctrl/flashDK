//! Live proof-of-life for the NanoKVM adapter: performs the AES login and opens the
//! HID WebSocket, then prints capabilities. Sends NO input — zero side effects.
//!
//! ```bash
//! NANOKVM_HOST=10.0.10.10 NANOKVM_USER=labored3640 NANOKVM_PASS='...' \
//!   cargo run -p flashdk-adapters --example nanokvm_demo
//! ```

use flashdk_adapters::nanokvm::NanoKvm;
use flashdk_core::Device;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("NANOKVM_HOST").expect("set NANOKVM_HOST");
    let user = std::env::var("NANOKVM_USER").expect("set NANOKVM_USER");
    let pass = std::env::var("NANOKVM_PASS").expect("set NANOKVM_PASS");

    println!("Logging in (AES) and opening HID WebSocket…");
    let kvm = NanoKvm::connect(host, &user, &pass).await?;
    println!(
        "Connected to {} ({:?})",
        kvm.info().model,
        kvm.info().vendor
    );
    println!("Transport: {:?}", kvm.transport_kind());
    println!("Capabilities: {:#?}", kvm.capabilities());
    println!("Login + WebSocket OK — no input was sent.");
    Ok(())
}
