//! IntfsOrch - Router interface orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation (intfsorch.cpp, 1,782 lines) has critical safety issues:
//! - Manual ref count increment/decrement with no underflow protection (lines 178-194)
//! - Complex nested loops for IP overlap detection (lines 557-582)
//! - No transaction semantics for VRF changes (lines 850-862)
//! - Unchecked ref_count before deletion (line 1327)
//!
//! The Rust implementation uses:
//! - Checked arithmetic for reference counting
//! - IP overlap validator with clear error messages
//! - Transactional VRF updates
//! - Type-safe RIF type enum

mod types;

pub use types::{IntfsEntry, RifType};
