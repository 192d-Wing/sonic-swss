//! StpOrch - Spanning Tree Protocol orchestration for SONiC.
//!
//! This module manages STP instances and port states in SONiC's switch abstraction layer.
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation (stporch.cpp, 615 lines) has several safety issues:
//! - Unchecked map access with `[]` operator (lines 215, 268)
//! - `std::stoi()` exceptions not caught (lines 400, 446, 464, 543)
//! - Return value bug in `updateStpPortState` (line 346 - returns true on error)
//! - Early returns without erasing iterator (lines 442, 451)
//! - Global mutable state via extern pointers
//! - No validation of instance ID ranges
//!
//! The Rust implementation uses:
//! - Safe map access with `.get()` returning `Option<T>`
//! - `.parse::<u16>()` returning `Result` for all string parsing
//! - Proper error propagation with `Result<T, E>`
//! - All C++ logic bugs fixed (correct return values, proper iterator handling)
//! - Type-safe STP state conversions
//! - Validated instance ID ranges

mod ffi;
mod orch;
mod types;

pub use ffi::{register_stp_orch, unregister_stp_orch};
pub use orch::{StpOrch, StpOrchCallbacks, StpOrchConfig, StpOrchError, StpOrchStats};
pub use types::{SaiStpPortState, StpInstanceEntry, StpPortIds, StpState};
