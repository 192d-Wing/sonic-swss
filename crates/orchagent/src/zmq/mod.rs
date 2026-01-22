//! ZmqOrch - ZeroMQ messaging orchestration for SONiC event notification.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Vec for message payloads instead of raw buffers
//! - String for topics and endpoints
//! - Option for optional endpoint configuration

mod ffi;
mod orch;
mod types;

pub use ffi::{register_zmq_orch, unregister_zmq_orch};
pub use orch::{ZmqOrch, ZmqOrchCallbacks, ZmqOrchConfig, ZmqOrchError, ZmqOrchStats};
pub use types::{ZmqEndpoint, ZmqMessage, ZmqStats};
