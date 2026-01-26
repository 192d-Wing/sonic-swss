//! gRPC API interface for neighsyncd
//!
//! Provides gRPC endpoints for remote management and monitoring of neighsyncd.
//! Implements services for:
//! - Neighbor CRUD operations
//! - State queries
//! - Health monitoring
//! - Configuration management

use crate::error::Result;
use crate::types::NeighborEntry;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

/// Neighbor information for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborInfo {
    /// IP address (v4/v6)
    pub ip_address: String,
    /// MAC address
    pub mac_address: String,
    /// Interface name
    pub interface: String,
    /// Neighbor state
    pub state: String,
    /// Family (IPv4 or IPv6)
    pub family: String,
}

impl NeighborInfo {
    /// Create from NeighborEntry
    pub fn from_entry(entry: &NeighborEntry) -> Self {
        Self {
            ip_address: entry.ip.to_string(),
            mac_address: entry.mac.to_string(),
            interface: entry.interface.clone(),
            state: format!("{:?}", entry.state),
            family: entry.family_str().to_string(),
        }
    }
}

/// Health status info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthInfo {
    /// Is daemon healthy
    pub is_healthy: bool,
    /// Health status description
    pub status: String,
    /// Time since last event (seconds)
    pub time_since_last_event: u64,
    /// Event processing latency (milliseconds)
    pub latency_ms: f64,
    /// Neighbors processed count
    pub neighbors_processed: u64,
    /// Errors encountered
    pub error_count: u64,
}

/// Statistics info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsInfo {
    /// Total neighbors processed
    pub total_neighbors: u64,
    /// Total events processed
    pub total_events: u64,
    /// Failed events
    pub failed_events: u64,
    /// Netlink errors
    pub netlink_errors: u64,
    /// Redis errors
    pub redis_errors: u64,
    /// Uptime (seconds)
    pub uptime_secs: u64,
}

/// Configuration info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigInfo {
    /// Redis host
    pub redis_host: String,
    /// Redis port
    pub redis_port: u16,
    /// Batch size
    pub batch_size: u64,
    /// Batch timeout (ms)
    pub batch_timeout_ms: u64,
    /// Warm restart enabled
    pub warm_restart_enabled: bool,
    /// IPv4 support enabled
    pub ipv4_enabled: bool,
}

impl Default for ConfigInfo {
    fn default() -> Self {
        Self {
            redis_host: "127.0.0.1".to_string(),
            redis_port: 6379,
            batch_size: 100,
            batch_timeout_ms: 100,
            warm_restart_enabled: true,
            ipv4_enabled: false,
        }
    }
}

/// Query parameters for neighbor listing
#[derive(Debug, Clone, Default)]
pub struct QueryParams {
    /// Filter by interface name
    pub interface: Option<String>,
    /// Filter by state
    pub state: Option<String>,
    /// Filter by family (IPv4 or IPv6)
    pub family: Option<String>,
    /// Limit results
    pub limit: Option<u32>,
}

/// API service trait for neighsyncd operations
pub trait NeighsyncService: Send + Sync {
    /// Get all neighbors
    fn get_neighbors(&self, query: &QueryParams) -> Result<Vec<NeighborInfo>>;

    /// Get specific neighbor
    fn get_neighbor(&self, ip_address: &str) -> Result<NeighborInfo>;

    /// Add or update neighbor
    fn add_neighbor(&self, neighbor: &NeighborInfo) -> Result<()>;

    /// Delete neighbor
    fn delete_neighbor(&self, ip_address: &str) -> Result<()>;

    /// Get health status
    fn get_health(&self) -> Result<HealthInfo>;

    /// Get statistics
    fn get_stats(&self) -> Result<StatsInfo>;

    /// Get configuration
    fn get_config(&self) -> Result<ConfigInfo>;

    /// Update configuration
    fn update_config(&self, config: &ConfigInfo) -> Result<()>;

    /// Restart daemon
    fn restart(&self) -> Result<()>;

    /// Get daemon status
    fn get_status(&self) -> Result<String>;
}

/// Mock implementation for testing
pub struct MockNeighsyncService {
    neighbors: Arc<parking_lot::Mutex<Vec<NeighborInfo>>>,
}

impl MockNeighsyncService {
    /// Create new mock service
    pub fn new() -> Self {
        Self {
            neighbors: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }
}

impl Default for MockNeighsyncService {
    fn default() -> Self {
        Self::new()
    }
}

impl NeighsyncService for MockNeighsyncService {
    fn get_neighbors(&self, _query: &QueryParams) -> Result<Vec<NeighborInfo>> {
        let neighbors = self.neighbors.lock();
        Ok(neighbors.clone())
    }

    fn get_neighbor(&self, ip_address: &str) -> Result<NeighborInfo> {
        let neighbors = self.neighbors.lock();
        neighbors
            .iter()
            .find(|n| n.ip_address == ip_address)
            .cloned()
            .ok_or_else(|| {
                crate::error::NeighsyncError::Config(format!("Neighbor {} not found", ip_address))
            })
    }

    fn add_neighbor(&self, neighbor: &NeighborInfo) -> Result<()> {
        let mut neighbors = self.neighbors.lock();
        neighbors.push(neighbor.clone());
        info!(ip = %neighbor.ip_address, "Added neighbor via API");
        Ok(())
    }

    fn delete_neighbor(&self, ip_address: &str) -> Result<()> {
        let mut neighbors = self.neighbors.lock();
        neighbors.retain(|n| n.ip_address != ip_address);
        info!(ip = ip_address, "Deleted neighbor via API");
        Ok(())
    }

    fn get_health(&self) -> Result<HealthInfo> {
        Ok(HealthInfo {
            is_healthy: true,
            status: "healthy".to_string(),
            time_since_last_event: 0,
            latency_ms: 1.5,
            neighbors_processed: 1000,
            error_count: 0,
        })
    }

    fn get_stats(&self) -> Result<StatsInfo> {
        Ok(StatsInfo {
            total_neighbors: 100,
            total_events: 1000,
            failed_events: 0,
            netlink_errors: 0,
            redis_errors: 0,
            uptime_secs: 3600,
        })
    }

    fn get_config(&self) -> Result<ConfigInfo> {
        Ok(ConfigInfo::default())
    }

    fn update_config(&self, _config: &ConfigInfo) -> Result<()> {
        info!("Configuration updated via API");
        Ok(())
    }

    fn restart(&self) -> Result<()> {
        info!("Daemon restart requested via API");
        Ok(())
    }

    fn get_status(&self) -> Result<String> {
        Ok("running".to_string())
    }
}

/// REST API server configuration
#[derive(Debug, Clone)]
pub struct RestServerConfig {
    /// Listen address
    pub listen_addr: SocketAddr,
    /// Enable CORS
    pub enable_cors: bool,
    /// Request timeout (seconds)
    pub request_timeout_secs: u64,
}

impl Default for RestServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:8080".parse().expect("Valid address"),
            enable_cors: false,
            request_timeout_secs: 30,
        }
    }
}

/// API error response
#[derive(Debug, Clone)]
pub struct ApiError {
    /// Error code
    pub code: u32,
    /// Error message
    pub message: String,
    /// Details
    pub details: Option<String>,
}

impl ApiError {
    /// Create new API error
    pub fn new(code: u32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    /// Add details to error
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neighbor_info_creation() {
        let info = NeighborInfo {
            ip_address: "fe80::1".to_string(),
            mac_address: "00:11:22:33:44:55".to_string(),
            interface: "eth0".to_string(),
            state: "reachable".to_string(),
            family: "IPv6".to_string(),
        };

        assert_eq!(info.ip_address, "fe80::1");
        assert_eq!(info.interface, "eth0");
    }

    #[test]
    fn test_health_info() {
        let health = HealthInfo {
            is_healthy: true,
            status: "healthy".to_string(),
            time_since_last_event: 5,
            latency_ms: 2.5,
            neighbors_processed: 500,
            error_count: 0,
        };

        assert!(health.is_healthy);
        assert_eq!(health.error_count, 0);
    }

    #[test]
    fn test_stats_info() {
        let stats = StatsInfo {
            total_neighbors: 100,
            total_events: 1000,
            failed_events: 5,
            netlink_errors: 1,
            redis_errors: 2,
            uptime_secs: 7200,
        };

        assert_eq!(stats.total_neighbors, 100);
        assert_eq!(stats.uptime_secs, 7200);
    }

    #[test]
    fn test_config_info_default() {
        let config = ConfigInfo::default();
        assert_eq!(config.redis_host, "127.0.0.1");
        assert_eq!(config.redis_port, 6379);
        assert_eq!(config.batch_size, 100);
    }

    #[test]
    fn test_query_params_default() {
        let params = QueryParams::default();
        assert!(params.interface.is_none());
        assert!(params.state.is_none());
    }

    #[test]
    fn test_mock_service_add_neighbor() {
        let service = MockNeighsyncService::new();
        let neighbor = NeighborInfo {
            ip_address: "fe80::1".to_string(),
            mac_address: "00:11:22:33:44:55".to_string(),
            interface: "eth0".to_string(),
            state: "reachable".to_string(),
            family: "IPv6".to_string(),
        };

        service.add_neighbor(&neighbor).unwrap();
        let retrieved = service.get_neighbor("fe80::1").unwrap();
        assert_eq!(retrieved.ip_address, "fe80::1");
    }

    #[test]
    fn test_mock_service_delete_neighbor() {
        let service = MockNeighsyncService::new();
        let neighbor = NeighborInfo {
            ip_address: "fe80::1".to_string(),
            mac_address: "00:11:22:33:44:55".to_string(),
            interface: "eth0".to_string(),
            state: "reachable".to_string(),
            family: "IPv6".to_string(),
        };

        service.add_neighbor(&neighbor).unwrap();
        service.delete_neighbor("fe80::1").unwrap();

        let result = service.get_neighbor("fe80::1");
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_service_get_health() {
        let service = MockNeighsyncService::new();
        let health = service.get_health().unwrap();

        assert!(health.is_healthy);
        assert_eq!(health.status, "healthy");
    }

    #[test]
    fn test_mock_service_get_stats() {
        let service = MockNeighsyncService::new();
        let stats = service.get_stats().unwrap();

        assert_eq!(stats.total_neighbors, 100);
        assert_eq!(stats.failed_events, 0);
    }

    #[test]
    fn test_api_error_creation() {
        let error = ApiError::new(404, "Not found");
        assert_eq!(error.code, 404);
        assert_eq!(error.message, "Not found");
    }

    #[test]
    fn test_api_error_with_details() {
        let error = ApiError::new(500, "Server error").with_details("Database connection failed");
        assert_eq!(error.code, 500);
        assert!(error.details.is_some());
        assert_eq!(error.details.unwrap(), "Database connection failed");
    }

    #[test]
    fn test_rest_server_config_default() {
        let config = RestServerConfig::default();
        assert_eq!(config.listen_addr.port(), 8080);
        assert_eq!(config.request_timeout_secs, 30);
    }

    #[test]
    fn test_mock_service_get_config() {
        let service = MockNeighsyncService::new();
        let config = service.get_config().unwrap();

        assert_eq!(config.redis_host, "127.0.0.1");
        assert!(!config.ipv4_enabled);
    }

    #[test]
    fn test_mock_service_restart() {
        let service = MockNeighsyncService::new();
        let result = service.restart();
        assert!(result.is_ok());
    }

    #[test]
    fn test_mock_service_get_status() {
        let service = MockNeighsyncService::new();
        let status = service.get_status().unwrap();
        assert_eq!(status, "running");
    }
}
