//! MirrorOrch - Port mirroring orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Safe map access with `.get()` returning `Option<T>`
//! - Saturating reference counting preventing underflow
//! - Type-safe IP family matching at compile time
//! - Validated DSCP and queue range checks

mod ffi;
mod orch;
pub mod types;

pub use ffi::{register_mirror_orch, unregister_mirror_orch};
pub use orch::{MirrorOrch, MirrorOrchCallbacks, MirrorOrchConfig, MirrorOrchError, MirrorOrchStats};
pub use types::{MirrorDirection, MirrorEntry, MirrorSessionConfig, MirrorSessionType};
