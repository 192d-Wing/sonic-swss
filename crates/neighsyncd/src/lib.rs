//! Neighbor Synchronization Daemon for SONiC
//!
//! This crate provides the Rust implementation of neighsyncd, which synchronizes
//! the kernel neighbor table (NDP for IPv6, ARP for IPv4 if enabled) to SONiC's
//! Redis databases.
//!
//! # Features
//!
//! - **default**: IPv6-only (NDP) neighbor synchronization
//! - **ipv4**: Enable IPv4/ARP neighbor support
//! - **dual-stack**: Enable both IPv4 and IPv6
//!
//! # NIST 800-53 Rev 5 Control Mappings
//!
//! This module implements the following security controls:
//!
//! | Control | Description | Implementation |
//! |---------|-------------|----------------|
//! | AC-3 | Access Enforcement | Kernel netlink requires CAP_NET_ADMIN |
//! | AU-3 | Content of Audit Records | Structured logging with neighbor details |
//! | AU-12 | Audit Record Generation | All neighbor changes logged |
//! | CM-6 | Configuration Settings | Configurable via Redis CONFIG_DB |
//! | CM-8 | System Component Inventory | Track network neighbors |
//! | CP-10 | System Recovery | Warm restart support |
//! | IA-3 | Device Identification | MAC address tracking |
//! | SC-5 | DoS Protection | Filter broadcast/multicast |
//! | SC-7 | Boundary Protection | Network boundary awareness |
//! | SC-8 | Transmission Confidentiality | Redis connection security |
//! | SI-4 | System Monitoring | Real-time neighbor monitoring |
//! | SI-10 | Input Validation | Validate neighbor entries |
//! | SI-11 | Error Handling | Structured error types |
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
//! │  Linux Kernel   │     │   neighsyncd    │     │  Redis (SONiC)  │
//! │                 │     │                 │     │                 │
//! │  Neighbor Table │────▶│   NetlinkSocket │────▶│    APPL_DB      │
//! │  (IPv6 NDP)     │     │        │        │     │   NEIGH_TABLE   │
//! │                 │     │        ▼        │     │                 │
//! │  RTM_NEWNEIGH   │     │   NeighSync     │◀───▶│   CONFIG_DB     │
//! │  RTM_DELNEIGH   │     │        │        │     │   (link-local)  │
//! │                 │     │        ▼        │     │                 │
//! └─────────────────┘     │  RedisAdapter   │     │   STATE_DB      │
//!                         │                 │     │   (warm restart)│
//!                         └─────────────────┘     └─────────────────┘
//! ```

pub mod advanced_health;
pub mod error;
pub mod health_monitor;
pub mod metrics;
pub mod metrics_server;
pub mod neigh_sync;
pub mod netlink;
pub mod redis_adapter;
pub mod tracing_integration;
pub mod types;

pub use advanced_health::{
    AdvancedHealthMonitor, DependencyHealth, HealthStatus, HealthThresholds, PerformanceMetrics,
};
pub use error::{NeighsyncError, Result};
pub use health_monitor::HealthMonitor;
pub use metrics::{HealthStatus as MetricsHealthStatus, MetricsCollector};
pub use metrics_server::{
    MetricsServerConfig, start_metrics_server, start_metrics_server_insecure,
};
pub use neigh_sync::{AsyncNeighSync, NeighSync};
pub use netlink::{AsyncNetlinkSocket, NetlinkSocket};
pub use redis_adapter::RedisAdapter;
pub use tracing_integration::{Span, SpanKind, SpanStatus, TracingIntegration};
pub use types::{MacAddress, NeighborEntry, NeighborMessageType, NeighborState};
