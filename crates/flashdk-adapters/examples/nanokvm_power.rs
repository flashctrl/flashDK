//! Live power-button check for the NanoKVM: reads the state, sends ONE action, then
//! polls the state for 20 s. This really presses the host's power button.
//!
//! ```bash
//! NANOKVM_HOST=192.0.2.11 NANOKVM_TOKEN='...' POWER_ACTION=short \
//!   cargo run -p flashdk-adapters --example nanokvm_power
//! ```
//! `POWER_ACTION` is one of `short`, `long`, `reset`, `on`.

use std::sync::Arc;
use std::time::Duration;

use flashdk_adapters::nanokvm::NanoKvm;
use flashdk_adapters::tls_pin::MemoryPinStore;
use flashdk_core::power::{Power, PowerAction};
use flashdk_core::TlsPolicy;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("NANOKVM_HOST").expect("set NANOKVM_HOST");
    let token = std::env::var("NANOKVM_TOKEN").expect("set NANOKVM_TOKEN");
    let action = match std::env::var("POWER_ACTION").as_deref() {
        Ok("short") => PowerAction::ShortPress,
        Ok("long") => PowerAction::LongPress,
        Ok("reset") => PowerAction::Reset,
        Ok("on") => PowerAction::On,
        _ => panic!("set POWER_ACTION to short|long|reset|on"),
    };
    let kvm = NanoKvm::connect_with_token(
        host,
        &token,
        TlsPolicy::PreferTls,
        Arc::new(MemoryPinStore::default()),
    )
    .await?;

    println!("before: {:?}", kvm.state().await?);
    println!("sending {action:?}");
    kvm.action(action).await?;
    for i in 1..=10 {
        tokio::time::sleep(Duration::from_secs(2)).await;
        println!("t+{:>2}s: {:?}", i * 2, kvm.state().await);
    }
    Ok(())
}
