//! Read-only proof-of-life for the PiKVM adapter: connect, then print capabilities,
//! power state, and available virtual-media images. Sends NO input and triggers no
//! power action, safe regardless of whether an ATX controller is attached.
//!
//! ```bash
//! PIKVM_HOST=10.0.10.20 PIKVM_USER=admin PIKVM_PASS='...' \
//!   cargo run -p flashdk-adapters --example pikvm_demo
//! ```

use flashdk_adapters::pikvm::PiKvm;
use flashdk_core::media::VirtualMedia;
use flashdk_core::power::Power;
use flashdk_core::Device;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("PIKVM_HOST").expect("set PIKVM_HOST");
    let user = std::env::var("PIKVM_USER").expect("set PIKVM_USER");
    let pass = std::env::var("PIKVM_PASS").expect("set PIKVM_PASS");

    let kvm = PiKvm::new(host, user, pass)?;
    println!(
        "Before refresh: {} ({:?})",
        kvm.info().model,
        kvm.info().vendor
    );
    if let Err(e) = kvm.refresh_identity().await {
        println!("refresh_identity failed: {e}");
    }
    println!(
        "After refresh:  {} (firmware {})",
        kvm.info().model,
        kvm.info().firmware
    );

    match kvm.state().await {
        Ok(s) => println!("Power state: {:?}", s),
        Err(e) => println!("Power state: unavailable ({e})"),
    }

    match kvm.list().await {
        Ok(images) => {
            println!("Virtual-media images ({}):", images.len());
            for img in images {
                let mb = img.size.map(|b| b / 1_000_000).unwrap_or(0);
                println!(
                    "  {}{}  ({} MB)",
                    if img.mounted { "* " } else { "  " },
                    img.name,
                    mb
                );
            }
        }
        Err(e) => println!("Media list: unavailable ({e})"),
    }
    Ok(())
}
