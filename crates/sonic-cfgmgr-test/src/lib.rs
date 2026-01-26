//! Integration test infrastructure for SONiC configuration managers
//!
//! Provides:
//! - Mock Redis database setup
//! - Test fixtures for common patterns
//! - CONFIG_DB change simulation
//! - APPL_DB verification helpers
//! - Multi-manager interaction tests

pub mod fixtures;
mod redis_env;
mod verification;

pub use fixtures::*;
pub use redis_env::RedisTestEnv;
pub use verification::*;
