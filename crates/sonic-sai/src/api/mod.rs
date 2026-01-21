//! Safe wrappers for SAI API functions.
//!
//! Each submodule provides type-safe Rust wrappers around the corresponding
//! SAI C API. These wrappers:
//!
//! - Use type-safe object IDs to prevent mixing different object types
//! - Convert SAI status codes to Rust Results
//! - Provide safe abstractions over raw pointers
//!
//! # Available API Modules
//!
//! - [`port`]: Port configuration and management
//! - [`route`]: Route and next-hop management
//! - [`switch`]: Switch-level configuration
//! - [`vlan`]: VLAN management
//! - [`acl`]: ACL table and rule management
//! - [`neighbor`]: Neighbor entry management
//! - [`fdb`]: FDB (MAC address table) management
//! - [`buffer`]: Buffer pool and profile management

pub mod port;
pub mod route;

// Re-export commonly used items
pub use port::PortApi;
pub use route::RouteApi;
