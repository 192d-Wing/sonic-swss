//! Buffer Manager Daemon - Buffer profile and PG configuration manager
//!
//! buffermgrd manages buffer profiles and buffer priority group (PG) assignments
//! based on port speed, cable length, and Priority Flow Control (PFC) configuration.
//!
//! Key features:
//! - Parse PG profile lookup file (speed/cable → buffer parameters)
//! - Monitor port speed, cable length, and PFC enable changes
//! - Dynamically generate buffer profiles
//! - Create buffer PG assignments for lossless priority groups
//! - Platform-specific handling (Mellanox, Barefoot)

pub mod buffer_mgr;
pub mod pg_bitmap;
pub mod pg_lookup;
pub mod tables;
pub mod types;

pub use buffer_mgr::BufferMgr;
pub use pg_lookup::parse_pg_lookup_file;
pub use types::PgProfile;
