//! Port Synchronization Daemon
//!
//! Synchronizes kernel port/interface status with SONiC databases via netlink events.
//! Listens for RTM_NEWLINK and RTM_DELLINK messages and updates STATE_DB with port status.
//!
//! NIST 800-53 Rev5 [SC-7]: Boundary Protection - Port status synchronization
//! NIST 800-53 Rev5 [SI-4]: System Monitoring - Real-time port state monitoring

pub mod config;
pub mod config_file;
pub mod eoiu_detector;
pub mod error;
pub mod metrics;
pub mod metrics_exporter;
pub mod metrics_server;
pub mod netlink_socket;
pub mod performance;
pub mod port_sync;
pub mod production_db;
pub mod production_features;
pub mod redis_adapter;
pub mod warm_restart;

pub use config::*;
pub use config_file::{HealthConfig, PerformanceConfig, PortsyncConfig};
pub use eoiu_detector::{EoiuDetectionState, EoiuDetector};
pub use error::*;
pub use metrics::MetricsCollector;
pub use metrics_exporter::PrometheusExporter;
pub use metrics_server::{MetricsServer, MetricsServerConfig, spawn_metrics_server};
pub use netlink_socket::NetlinkSocket;
pub use performance::{BenchmarkConfig, BenchmarkResult, PerformanceMetrics};
pub use port_sync::*;
pub use production_db::ProductionDatabase;
pub use production_features::{HealthMonitor, ShutdownCoordinator, SystemdNotifier};
pub use redis_adapter::RedisAdapter;
pub use warm_restart::{
    PersistedPortState, PortState, WarmRestartManager, WarmRestartMetrics, WarmRestartState,
};
