//! FFI bridge for C++/Rust interoperability during migration.
//!
//! This crate provides the glue layer between Rust orchagent modules and
//! the existing C++ codebase. It enables:
//!
//! - Rust code to call into C++ Orchs (e.g., accessing gPortsOrch)
//! - C++ code to call into Rust Orchs (as modules are migrated)
//!
//! # Migration Strategy
//!
//! During the migration period, both Rust and C++ code coexist:
//!
//! ```text
//! Phase 1: Rust modules call C++ via FFI
//! [Rust RouteOrch] --FFI--> [C++ gPortsOrch, gNeighOrch]
//!
//! Phase 2: Hybrid - some Rust, some C++
//! [Rust RouteOrch] --FFI--> [Rust PortsOrch]
//! [Rust RouteOrch] --FFI--> [C++ gNeighOrch]
//!
//! Phase 3: All Rust with C++ shim for external callers
//! [Rust RouteOrch] --> [Rust PortsOrch]
//! [C++ syncd] --FFI--> [Rust orchagent]
//! ```
//!
//! # Safety
//!
//! All FFI functions in this crate use `extern "C"` ABI and follow
//! these safety rules:
//!
//! 1. Pointers passed to/from C++ are validated before use
//! 2. Strings are handled via null-terminated C strings
//! 3. Object lifetimes are carefully managed to prevent use-after-free
//! 4. Thread safety is ensured via appropriate synchronization

mod cpp_bridge;
mod rust_exports;

pub use cpp_bridge::*;
pub use rust_exports::*;
