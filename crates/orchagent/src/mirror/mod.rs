//! MirrorOrch - Port mirroring orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation (mirrororch.cpp, 1,611 lines) has critical safety issues:
//! - Unchecked map access with find() (lines 220, 234, 248)
//! - Reference count underflow (lines 264-269)
//! - IP family validation only at runtime (line 494)
//! - Manual OID list management with raw pointers
//!
//! The Rust implementation uses:
//! - Safe map access with `.get()` returning `Option<T>`
//! - Saturating reference counting preventing underflow
//! - Type-safe IP family matching at compile time
//! - Validated DSCP and queue range checks

mod types;

pub use types::{MirrorDirection, MirrorEntry, MirrorSessionConfig, MirrorSessionType};
