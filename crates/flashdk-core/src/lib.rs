//! # flashdk-core
//!
//! The vendor-neutral heart of the SDK. It defines *what a KVM can do* as a set of
//! small Rust traits (contracts) and plain data types — and deliberately knows
//! nothing about NanoKVM, PiKVM, or JetKVM. Each vendor's quirks live in
//! `flashdk-adapters`, which implements these traits.
//!
//! ## Why it's shaped this way
//!
//! Our live probing showed the four devices split along two axes:
//!
//! * **Some capabilities unify cleanly** (keyboard/mouse, virtual media) — every
//!   device means the same thing, just with a different envelope. Those become
//!   shared traits here.
//! * **Some are structurally different** (video, and the whole transport model).
//!   JetKVM carries control as JSON-RPC over a WebRTC DataChannel, while PiKVM and
//!   NanoKVM use plain HTTP request/response. We model that difference explicitly in
//!   [`transport::TransportKind`] instead of pretending it away.
//!
//! A new learner's reading order: `capability` → `transport` → `hid` → `device`.
//!
//! ## Beyond KVMs: `outlet` and `ups`
//!
//! [`outlet`] and [`ups`] cover a different device class (networked PDUs and UPS
//! units) that shares the capability-based design but not the [`Device`] umbrella
//! trait: those devices have no keyboard, mouse, or video, so forcing them through
//! `Hid`/`Power`/`VirtualMedia` would mean every method returning `NotSupported`.
//! See either module's doc comment for the reasoning.

// Native `async fn` in traits is stable, but the compiler warns when they appear in
// *public* traits (it wants you to think about auto-trait bounds on the returned
// future). For a scaffold that's just noise, so we silence it crate-wide and will
// revisit when the real networking lands.
#![allow(async_fn_in_trait)]

pub mod capability;
pub mod device;
pub mod error;
pub mod hid;
pub mod media;
pub mod outlet;
pub mod power;
pub mod transport;
pub mod ups;

// Re-export the handful of types callers reach for most, so app code can write
// `use flashdk_core::{Kvm, Vendor, Result};` without spelunking submodules.
pub use capability::{Capabilities, Vendor};
pub use device::{Device, DeviceInfo};
pub use error::{Error, Result};
pub use transport::TransportKind;
