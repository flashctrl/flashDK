//! Intel vPro AMT power control via WS-Management, built entirely from three
//! official DMTF specifications (DSP0226, DSP0227, DSP0230) plus Intel's own AMT
//! SDK documentation; see `PROVENANCE.md` for exactly which fact traces to which
//! document.
//!
//! **No live client yet.** `protocol.rs` builds and unit-tests the SOAP envelope
//! for `CIM_PowerManagementService.RequestPowerStateChange` against the specs'
//! own literal rules and examples, but there is no HTTP transport or
//! Digest/Kerberos authentication here: AMT's authentication handshake needs a
//! live capture against a provisioned AMT 3.0+ unit to source honestly, and this
//! project's one real vPro-capable unit currently reports AMT 1.2, which predates
//! WS-Management entirely (see `docs/ROADMAP.md`). Treat this module the way
//! `docs/STATE.md` treats other pre-hardware work: real, spec-derived code, not
//! yet proven against anything that talks back.

pub mod protocol;
