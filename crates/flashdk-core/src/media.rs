//! Virtual media: present an ISO/image to the target as a USB drive or CD-ROM.
//! Unifies cleanly: every device models it as list, mount, unmount, plus state.

use crate::error::Result;

/// One image known to the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaImage {
    pub name: String,
    /// Size in bytes if the device reports it.
    pub size: Option<u64>,
    /// Whether this image is the one currently mounted.
    pub mounted: bool,
}

/// The virtual-media contract adapters implement.
pub trait VirtualMedia {
    /// List images already present on the device.
    async fn list(&self) -> Result<Vec<MediaImage>>;
    /// Mount an image by name so the target sees it as a drive.
    async fn mount(&self, name: &str) -> Result<()>;
    /// Unmount whatever is currently presented.
    async fn unmount(&self) -> Result<()>;
}
