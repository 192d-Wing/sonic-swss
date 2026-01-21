//! Common orchestration abstractions for SONiC.
//!
//! This crate provides the core traits and types used by all orchestration
//! modules in the SONiC control plane:
//!
//! - [`Orch`]: Base trait for orchestration agents
//! - [`Consumer`]: Trait for consuming table entries from Redis
//! - [`SyncMap`]: Type-safe map that prevents auto-vivification bugs
//! - [`TaskStatus`]: Result type for task processing
//!
//! # Architecture
//!
//! The orchestration architecture follows an event-driven model:
//!
//! 1. Configuration changes are written to Redis (CONFIG_DB, APPL_DB)
//! 2. Orch modules subscribe to relevant tables via Consumers
//! 3. The OrchDaemon event loop dispatches tasks to appropriate Orchs
//! 4. Orchs translate configuration into SAI API calls
//! 5. State is written back to STATE_DB
//!
//! # Example
//!
//! ```ignore
//! use sonic_orch_common::{Orch, Consumer, TaskStatus};
//!
//! struct MyOrch {
//!     port_consumer: Consumer,
//!     // ... state
//! }
//!
//! #[async_trait]
//! impl Orch for MyOrch {
//!     fn name(&self) -> &str { "MyOrch" }
//!
//!     async fn do_task(&mut self) {
//!         for entry in self.port_consumer.drain() {
//!             match self.process_entry(entry).await {
//!                 Ok(()) => {}
//!                 Err(TaskStatus::NeedRetry) => self.port_consumer.retry(entry),
//!                 Err(e) => log::error!("Failed: {:?}", e),
//!             }
//!         }
//!     }
//! }
//! ```

mod orch;
mod consumer;
mod sync_map;
mod task;
mod retry;

pub use orch::{Orch, OrchContext};
pub use consumer::{Consumer, ConsumerConfig, KeyOpFieldsValues, Operation};
pub use sync_map::SyncMap;
pub use task::{TaskStatus, TaskResult};
pub use retry::{RetryCache, Constraint};
