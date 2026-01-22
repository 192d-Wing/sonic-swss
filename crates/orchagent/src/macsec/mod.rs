//! MacsecOrch - MACsec (802.1AE) orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Type-safe cipher suite enum
//! - Type-safe direction enum (Ingress/Egress)
//! - Validated Association Number (AN) range (0-3)
//! - Vec for key storage instead of raw buffers
//! - Type alias for SCI (Secure Channel Identifier)

mod ffi;
mod orch;
mod types;

pub use ffi::{register_macsec_orch, unregister_macsec_orch};
pub use orch::{MacsecOrch, MacsecOrchCallbacks, MacsecOrchConfig, MacsecOrchError, MacsecOrchStats};
pub use types::{
    MacsecCipherSuite, MacsecDirection, MacsecFlowEntry, MacsecPort,
    MacsecSa, MacsecSc, MacsecStats, Sci,
};
