//! Configuration file support for portsyncd
//!
//! Loads and validates portsyncd configuration from TOML files.
//! Default location: /etc/sonic/portsyncd.conf
//!
//! Phase 5 Week 5 implementation.

use crate::error::{PortsyncError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Redis host
    #[serde(default = "default_redis_host")]
    pub redis_host: String,

    /// Redis port
    #[serde(default = "default_redis_port")]
    pub redis_port: u16,

    /// Redis database number for CONFIG_DB
    #[serde(default = "default_config_db_number")]
    pub config_db_number: u32,

    /// Redis database number for STATE_DB
    #[serde(default = "default_state_db_number")]
    pub state_db_number: u32,

    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout_secs: u64,

    /// Connection retry interval in seconds
    #[serde(default = "default_retry_interval")]
    pub retry_interval_secs: u64,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Maximum event queue depth
    #[serde(default = "default_max_event_queue")]
    pub max_event_queue: usize,

    /// Batch processing timeout in milliseconds
    #[serde(default = "default_batch_timeout")]
    pub batch_timeout_ms: u64,

    /// Maximum latency target in microseconds
    #[serde(default = "default_max_latency")]
    pub max_latency_us: u64,

    /// Minimum success rate (percentage)
    #[serde(default = "default_min_success_rate")]
    pub min_success_rate: f64,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    /// Maximum stall duration in seconds before considering unhealthy
    #[serde(default = "default_max_stall_seconds")]
    pub max_stall_seconds: u64,

    /// Maximum failure rate (percentage) before degraded
    #[serde(default = "default_max_failure_rate")]
    pub max_failure_rate_percent: f64,

    /// Minimum port synchronization rate (percentage)
    #[serde(default = "default_min_port_sync_rate")]
    pub min_port_sync_rate: f64,

    /// Enable watchdog notifications
    #[serde(default = "default_enable_watchdog")]
    pub enable_watchdog: bool,

    /// Watchdog notification interval in seconds
    #[serde(default = "default_watchdog_interval")]
    pub watchdog_interval_secs: u64,
}

/// Export format for metrics
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MetricsExportFormat {
    /// Prometheus text format
    #[default]
    Prometheus,
    /// JSON format
    Json,
    /// Both formats
    Both,
}

impl std::fmt::Display for MetricsExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetricsExportFormat::Prometheus => write!(f, "prometheus"),
            MetricsExportFormat::Json => write!(f, "json"),
            MetricsExportFormat::Both => write!(f, "both"),
        }
    }
}

/// Metrics configuration (Phase 6 Week 4)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics persistence and export
    #[serde(default = "default_metrics_enabled")]
    pub enabled: bool,

    /// Auto-save metrics interval in seconds
    #[serde(default = "default_metrics_save_interval")]
    pub save_interval_secs: u64,

    /// Retention period in days (keep metrics for N days)
    #[serde(default = "default_metrics_retention_days")]
    pub retention_days: u64,

    /// Maximum metrics file size in megabytes (rotate when exceeded)
    #[serde(default = "default_metrics_max_file_size")]
    pub max_file_size_mb: u64,

    /// Export format (prometheus, json, both)
    #[serde(default)]
    pub export_format: MetricsExportFormat,

    /// Storage path for metrics files
    #[serde(default = "default_metrics_storage_path")]
    pub storage_path: String,
}

/// Complete portsyncd configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PortsyncConfig {
    /// Database configuration
    #[serde(default)]
    pub database: DatabaseConfig,

    /// Performance configuration
    #[serde(default)]
    pub performance: PerformanceConfig,

    /// Health check configuration
    #[serde(default)]
    pub health: HealthConfig,

    /// Metrics configuration (Week 4)
    #[serde(default)]
    pub metrics: MetricsConfig,
}

// Default functions
fn default_redis_host() -> String {
    "127.0.0.1".to_string()
}

fn default_redis_port() -> u16 {
    6379
}

fn default_config_db_number() -> u32 {
    4
}

fn default_state_db_number() -> u32 {
    6
}

fn default_connection_timeout() -> u64 {
    5
}

fn default_retry_interval() -> u64 {
    2
}

fn default_max_event_queue() -> usize {
    1000
}

fn default_batch_timeout() -> u64 {
    100
}

fn default_max_latency() -> u64 {
    10000
}

fn default_min_success_rate() -> f64 {
    99.0
}

fn default_max_stall_seconds() -> u64 {
    10
}

fn default_max_failure_rate() -> f64 {
    5.0
}

fn default_min_port_sync_rate() -> f64 {
    90.0
}

fn default_enable_watchdog() -> bool {
    true
}

fn default_watchdog_interval() -> u64 {
    15
}

fn default_metrics_enabled() -> bool {
    true
}

fn default_metrics_save_interval() -> u64 {
    300 // 5 minutes
}

fn default_metrics_retention_days() -> u64 {
    30 // Keep metrics for 30 days
}

fn default_metrics_max_file_size() -> u64 {
    100 // 100 MB before rotation
}

fn default_metrics_storage_path() -> String {
    "/var/lib/sonic/portsyncd/metrics".to_string()
}

// Default implementations
impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            redis_host: default_redis_host(),
            redis_port: default_redis_port(),
            config_db_number: default_config_db_number(),
            state_db_number: default_state_db_number(),
            connection_timeout_secs: default_connection_timeout(),
            retry_interval_secs: default_retry_interval(),
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_event_queue: default_max_event_queue(),
            batch_timeout_ms: default_batch_timeout(),
            max_latency_us: default_max_latency(),
            min_success_rate: default_min_success_rate(),
        }
    }
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            max_stall_seconds: default_max_stall_seconds(),
            max_failure_rate_percent: default_max_failure_rate(),
            min_port_sync_rate: default_min_port_sync_rate(),
            enable_watchdog: default_enable_watchdog(),
            watchdog_interval_secs: default_watchdog_interval(),
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: default_metrics_enabled(),
            save_interval_secs: default_metrics_save_interval(),
            retention_days: default_metrics_retention_days(),
            max_file_size_mb: default_metrics_max_file_size(),
            export_format: MetricsExportFormat::default(),
            storage_path: default_metrics_storage_path(),
        }
    }
}

impl MetricsConfig {
    /// Validate metrics configuration
    pub fn validate(&self) -> Result<()> {
        if self.save_interval_secs == 0 {
            return Err(PortsyncError::Configuration(
                "metrics save_interval_secs must be > 0".to_string(),
            ));
        }

        if self.retention_days == 0 {
            return Err(PortsyncError::Configuration(
                "metrics retention_days must be > 0".to_string(),
            ));
        }

        if self.max_file_size_mb == 0 {
            return Err(PortsyncError::Configuration(
                "metrics max_file_size_mb must be > 0".to_string(),
            ));
        }

        Ok(())
    }
}

impl PortsyncConfig {
    /// Load configuration from file, falling back to defaults if file not found
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        match fs::read_to_string(path) {
            Ok(content) => {
                let config = toml::from_str(&content).map_err(|e| {
                    PortsyncError::Configuration(format!(
                        "Failed to parse config file {}: {}",
                        path.display(),
                        e
                    ))
                })?;
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!(
                    "portsyncd: Config file {} not found, using defaults",
                    path.display()
                );
                Ok(Self::default())
            }
            Err(e) => Err(PortsyncError::Io(e)),
        }
    }

    /// Load from default location or defaults
    pub fn load() -> Result<Self> {
        Self::load_or_default("/etc/sonic/portsyncd.conf")
    }

    /// Save configuration to file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let content = toml::to_string_pretty(self).map_err(|e| {
            PortsyncError::Configuration(format!("Failed to serialize config: {}", e))
        })?;

        fs::write(path, content).map_err(PortsyncError::Io)?;

        Ok(())
    }

    /// Get connection timeout as Duration
    pub fn connection_timeout(&self) -> Duration {
        Duration::from_secs(self.database.connection_timeout_secs)
    }

    /// Get retry interval as Duration
    pub fn retry_interval(&self) -> Duration {
        Duration::from_secs(self.database.retry_interval_secs)
    }

    /// Get watchdog interval as Duration
    pub fn watchdog_interval(&self) -> Duration {
        Duration::from_secs(self.health.watchdog_interval_secs)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.database.redis_port == 0 {
            return Err(PortsyncError::Configuration(
                "redis_port must be > 0".to_string(),
            ));
        }

        if self.performance.min_success_rate < 0.0 || self.performance.min_success_rate > 100.0 {
            return Err(PortsyncError::Configuration(
                "min_success_rate must be 0-100".to_string(),
            ));
        }

        if self.health.max_failure_rate_percent < 0.0
            || self.health.max_failure_rate_percent > 100.0
        {
            return Err(PortsyncError::Configuration(
                "max_failure_rate_percent must be 0-100".to_string(),
            ));
        }

        if self.health.min_port_sync_rate < 0.0 || self.health.min_port_sync_rate > 100.0 {
            return Err(PortsyncError::Configuration(
                "min_port_sync_rate must be 0-100".to_string(),
            ));
        }

        // Validate metrics config
        self.metrics.validate()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PortsyncConfig::default();
        assert_eq!(config.database.redis_host, "127.0.0.1");
        assert_eq!(config.database.redis_port, 6379);
        assert_eq!(config.database.config_db_number, 4);
        assert_eq!(config.database.state_db_number, 6);
    }

    #[test]
    fn test_database_config_defaults() {
        let config = DatabaseConfig::default();
        assert_eq!(config.redis_host, "127.0.0.1");
        assert_eq!(config.redis_port, 6379);
        assert_eq!(config.connection_timeout_secs, 5);
        assert_eq!(config.retry_interval_secs, 2);
    }

    #[test]
    fn test_performance_config_defaults() {
        let config = PerformanceConfig::default();
        assert_eq!(config.max_event_queue, 1000);
        assert_eq!(config.batch_timeout_ms, 100);
        assert_eq!(config.max_latency_us, 10000);
        assert!(config.min_success_rate >= 99.0);
    }

    #[test]
    fn test_health_config_defaults() {
        let config = HealthConfig::default();
        assert_eq!(config.max_stall_seconds, 10);
        assert_eq!(config.max_failure_rate_percent, 5.0);
        assert!(config.enable_watchdog);
    }

    #[test]
    fn test_validate_valid_config() {
        let config = PortsyncConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_redis_port() {
        let mut config = PortsyncConfig::default();
        config.database.redis_port = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_success_rate() {
        let mut config = PortsyncConfig::default();
        config.performance.min_success_rate = 101.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_connection_timeout_duration() {
        let config = PortsyncConfig::default();
        assert_eq!(config.connection_timeout(), Duration::from_secs(5));
    }

    #[test]
    fn test_retry_interval_duration() {
        let config = PortsyncConfig::default();
        assert_eq!(config.retry_interval(), Duration::from_secs(2));
    }

    #[test]
    fn test_watchdog_interval_duration() {
        let config = PortsyncConfig::default();
        assert_eq!(config.watchdog_interval(), Duration::from_secs(15));
    }

    #[test]
    fn test_toml_serialization() {
        let config = PortsyncConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("redis_host"));
        assert!(toml_str.contains("127.0.0.1"));
    }

    #[test]
    fn test_toml_deserialization() {
        let toml_str = r#"
[database]
redis_host = "192.168.1.1"
redis_port = 6380

[performance]
max_event_queue = 2000
"#;
        let config: PortsyncConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.database.redis_host, "192.168.1.1");
        assert_eq!(config.database.redis_port, 6380);
        assert_eq!(config.performance.max_event_queue, 2000);
        // Unspecified values should use defaults
        assert_eq!(config.database.config_db_number, 4);
    }

    #[test]
    fn test_load_nonexistent_file_defaults() {
        let config = PortsyncConfig::load_or_default("/nonexistent/path.conf").unwrap();
        assert_eq!(config.database.redis_host, "127.0.0.1");
    }

    #[test]
    fn test_metrics_config_defaults() {
        let config = MetricsConfig::default();
        assert!(config.enabled);
        assert_eq!(config.save_interval_secs, 300);
        assert_eq!(config.retention_days, 30);
        assert_eq!(config.max_file_size_mb, 100);
        assert_eq!(config.export_format, MetricsExportFormat::Prometheus);
        assert_eq!(config.storage_path, "/var/lib/sonic/portsyncd/metrics");
    }

    #[test]
    fn test_metrics_config_validate_valid() {
        let config = MetricsConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_metrics_config_validate_zero_save_interval() {
        let config = MetricsConfig {
            save_interval_secs: 0,
            ..MetricsConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_metrics_config_validate_zero_retention() {
        let config = MetricsConfig {
            retention_days: 0,
            ..MetricsConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_metrics_config_validate_zero_file_size() {
        let config = MetricsConfig {
            max_file_size_mb: 0,
            ..MetricsConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_portsyncd_config_validate_includes_metrics() {
        let mut config = PortsyncConfig::default();
        config.metrics.save_interval_secs = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_metrics_export_format_display() {
        assert_eq!(MetricsExportFormat::Prometheus.to_string(), "prometheus");
        assert_eq!(MetricsExportFormat::Json.to_string(), "json");
        assert_eq!(MetricsExportFormat::Both.to_string(), "both");
    }

    #[test]
    fn test_metrics_config_toml_parsing() {
        let toml_str = r#"
[metrics]
enabled = true
save_interval_secs = 600
retention_days = 60
max_file_size_mb = 200
export_format = "json"
storage_path = "/custom/path/metrics"
"#;
        let config: PortsyncConfig = toml::from_str(toml_str).unwrap();
        assert!(config.metrics.enabled);
        assert_eq!(config.metrics.save_interval_secs, 600);
        assert_eq!(config.metrics.retention_days, 60);
        assert_eq!(config.metrics.max_file_size_mb, 200);
        assert_eq!(config.metrics.export_format, MetricsExportFormat::Json);
        assert_eq!(config.metrics.storage_path, "/custom/path/metrics");
    }
}
