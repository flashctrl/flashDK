//! Live virtual-media check for the NanoKVM: lists images, mounts the first one,
//! confirms it shows as mounted, unmounts it, and confirms it is gone. Refuses to run
//! if something is already mounted, so it never disturbs an existing mount.
//!
//! ```bash
//! NANOKVM_HOST=10.0.10.11 NANOKVM_TOKEN='...' \
//!   cargo run -p flashdk-adapters --example nanokvm_media
//! ```

use std::sync::Arc;

use flashdk_adapters::nanokvm::NanoKvm;
use flashdk_adapters::tls_pin::MemoryPinStore;
use flashdk_core::media::VirtualMedia;
use flashdk_core::TlsPolicy;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("NANOKVM_HOST").expect("set NANOKVM_HOST");
    let token = std::env::var("NANOKVM_TOKEN").expect("set NANOKVM_TOKEN");
    let kvm = NanoKvm::connect_with_token(
        host,
        &token,
        TlsPolicy::PreferTls,
        Arc::new(MemoryPinStore::default()),
    )
    .await?;

    let before = kvm.list().await?;
    println!("before: {before:?}");
    if before.iter().any(|m| m.mounted) {
        return Err("an image is already mounted; not touching it".into());
    }
    let first = before
        .first()
        .ok_or("no images on the device")?
        .name
        .clone();

    kvm.mount(&first).await?;
    let mounted = kvm.list().await?;
    println!("after mount: {mounted:?}");
    let ok_mounted = mounted.iter().any(|m| m.name == first && m.mounted);

    kvm.unmount().await?;
    let after = kvm.list().await?;
    println!("after unmount: {after:?}");
    let ok_unmounted = after.iter().all(|m| !m.mounted);

    println!("mount visible: {ok_mounted}, unmount visible: {ok_unmounted}");
    if ok_mounted && ok_unmounted {
        Ok(())
    } else {
        Err("media state did not change as expected".into())
    }
}
