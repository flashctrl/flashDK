//! Read-only proof-of-life for the GL.iNet Comet adapter: connect, then print
//! capabilities, power state, and available virtual-media images. Sends NO input
//! and triggers no power action, safe regardless of whether a host is attached to
//! the capture port.
//!
//! ```bash
//! GLINET_HOST=10.0.10.22 GLINET_USER=admin GLINET_PASS='...' \
//!   cargo run -p flashdk-adapters --example glinet_demo
//! ```

use flashdk_adapters::glinet::GlInetKvm;
use flashdk_core::media::VirtualMedia;
use flashdk_core::power::Power;
use flashdk_core::Device;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("GLINET_HOST").expect("set GLINET_HOST");
    let user = std::env::var("GLINET_USER").expect("set GLINET_USER");
    let pass = std::env::var("GLINET_PASS").expect("set GLINET_PASS");

    let kvm = GlInetKvm::connect(&host, &user, &pass).await?;
    println!(
        "Connected: {} (firmware {})",
        kvm.info().model,
        kvm.info().firmware
    );
    println!("Capabilities: {:?}", kvm.capabilities());

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

    kvm.logout().await?;
    println!("Logged out cleanly.");
    Ok(())
}
