//! Production features for portsyncd daemon
//!
//! Includes systemd integration, health checks, and graceful shutdown.
//! Phase 4 Week 2 Day 5 implementation.

use crate::error::{PortsyncError, Result};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Health check status
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthStatus {
    /// Daemon is healthy and operational
    Healthy,
    /// Daemon is degraded but still functional
    Degraded,
    /// Daemon is unhealthy and may need restart
    Unhealthy,
}

impl HealthStatus {
    /// Convert to string for systemd notification
    pub fn as_str(&self) -> &'static str {
        match self {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Unhealthy => "unhealthy",
        }
    }
}

/// Health check parameters
#[derive(Clone, Debug)]
pub struct HealthCheckConfig {
    /// Maximum allowed duration without event processing (indicates stall)
    pub max_stall_duration: Duration,
    /// Maximum allowed percentage of failed events
    pub max_failure_rate: f64,
    /// Minimum acceptable port synchronization success rate
    pub min_port_sync_rate: f64,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            max_stall_duration: Duration::from_secs(10),
            max_failure_rate: 5.0,    // 5% failures is degraded
            min_port_sync_rate: 90.0, // 90% port sync is degraded
        }
    }
}

/// Health monitor for portsyncd
#[derive(Clone, Debug)]
pub struct HealthMonitor {
    /// Current health status
    status: Arc<std::sync::Mutex<HealthStatus>>,
    /// Last event timestamp
    last_event: Arc<std::sync::Mutex<Instant>>,
    /// Configuration
    config: HealthCheckConfig,
}

impl HealthMonitor {
    /// Create new health monitor
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            status: Arc::new(std::sync::Mutex::new(HealthStatus::Healthy)),
            last_event: Arc::new(std::sync::Mutex::new(Instant::now())),
            config,
        }
    }

    /// Record an event (updates last event timestamp)
    pub fn record_event(&self) {
        if let Ok(mut last) = self.last_event.lock() {
            *last = Instant::now();
        }
    }

    /// Check current health status based on activity
    pub fn check_health(&self) -> HealthStatus {
        if let Ok(last) = self.last_event.lock() {
            let since_last_event = last.elapsed();
            if since_last_event > self.config.max_stall_duration {
                return HealthStatus::Unhealthy;
            }
        }

        if let Ok(status) = self.status.lock() {
            return *status;
        }

        HealthStatus::Healthy
    }

    /// Set health status
    pub fn set_status(&self, status: HealthStatus) {
        if let Ok(mut current) = self.status.lock() {
            *current = status;
        }
    }

    /// Get current status
    pub fn status(&self) -> HealthStatus {
        self.check_health()
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new(HealthCheckConfig::default())
    }
}

/// Systemd notification for notify-on-ready and watchdog
///
/// Sends notifications to systemd for service readiness, health status,
/// and watchdog keepalives. Enabled when run under systemd with Type=notify.
#[derive(Clone, Debug)]
pub struct SystemdNotifier {
    /// Is systemd socket available (NOTIFY_SOCKET env var set)?
    enabled: bool,
}

impl SystemdNotifier {
    /// Create new systemd notifier
    ///
    /// Checks for NOTIFY_SOCKET environment variable to determine if
    /// running under systemd with notify socket support.
    pub fn new() -> Self {
        let enabled = std::env::var("NOTIFY_SOCKET").is_ok();

        if enabled {
            eprintln!("portsyncd: Systemd notification socket detected");
        }

        Self { enabled }
    }

    /// Send READY notification to systemd
    ///
    /// Indicates daemon has completed initialization and is ready to accept requests.
    /// Used by systemd's notify service type to know when daemon is ready.
    pub fn notify_ready(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        sd_notify::notify(true, &[sd_notify::NotifyState::Ready]).map_err(|e| {
            PortsyncError::Other(format!("Failed to send READY notification: {}", e))
        })?;

        eprintln!("portsyncd: Sent READY notification to systemd");
        Ok(())
    }

    /// Send WATCHDOG notification to systemd
    ///
    /// Indicates daemon is still alive and functioning. Should be sent
    /// periodically (within WatchdogSec timeout) to prevent systemd restart.
    pub fn notify_watchdog(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        sd_notify::notify(true, &[sd_notify::NotifyState::Watchdog]).map_err(|e| {
            PortsyncError::Other(format!("Failed to send WATCHDOG notification: {}", e))
        })?;

        Ok(())
    }

    /// Send status message to systemd
    ///
    /// Sends operational status to systemd journal and systemctl output.
    pub fn notify_status(&self, message: &str) -> Result<()> {
        if !self.enabled {
            eprintln!("portsyncd: Status: {}", message);
            return Ok(());
        }

        sd_notify::notify(true, &[sd_notify::NotifyState::Status(message)]).map_err(|e| {
            PortsyncError::Other(format!("Failed to send STATUS notification: {}", e))
        })?;

        eprintln!("portsyncd: Status: {}", message);
        Ok(())
    }

    /// Check if systemd is available
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for SystemdNotifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Graceful shutdown coordinator
#[derive(Clone, Debug)]
pub struct ShutdownCoordinator {
    /// Global shutdown flag
    shutdown_requested: Arc<AtomicBool>,
    /// Shutdown timeout
    timeout: Duration,
}

impl ShutdownCoordinator {
    /// Create new shutdown coordinator
    pub fn new(timeout: Duration) -> Self {
        Self {
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            timeout,
        }
    }

    /// Request graceful shutdown
    pub fn request_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::Relaxed);
        eprintln!("portsyncd: Graceful shutdown requested");
    }

    /// Check if shutdown was requested
    pub fn should_shutdown(&self) -> bool {
        self.shutdown_requested.load(Ordering::Relaxed)
    }

    /// Get shutdown timeout
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_strings() {
        assert_eq!(HealthStatus::Healthy.as_str(), "healthy");
        assert_eq!(HealthStatus::Degraded.as_str(), "degraded");
        assert_eq!(HealthStatus::Unhealthy.as_str(), "unhealthy");
    }

    #[test]
    fn test_health_check_config_default() {
        let config = HealthCheckConfig::default();
        assert_eq!(config.max_stall_duration, Duration::from_secs(10));
        assert_eq!(config.max_failure_rate, 5.0);
    }

    #[test]
    fn test_health_monitor_creation() {
        let monitor = HealthMonitor::new(HealthCheckConfig::default());
        assert_eq!(monitor.status(), HealthStatus::Healthy);
    }

    #[test]
    fn test_health_monitor_record_event() {
        let monitor = HealthMonitor::new(HealthCheckConfig::default());
        monitor.record_event();
        assert_eq!(monitor.check_health(), HealthStatus::Healthy);
    }

    #[test]
    fn test_health_monitor_status_update() {
        let monitor = HealthMonitor::new(HealthCheckConfig::default());
        monitor.set_status(HealthStatus::Degraded);
        assert_eq!(monitor.status(), HealthStatus::Degraded);
    }

    #[test]
    fn test_systemd_notifier_creation() {
        let notifier = SystemdNotifier::new();
        assert!(!notifier.is_enabled());
    }

    #[test]
    fn test_systemd_notify_ready() {
        let notifier = SystemdNotifier::new();
        assert!(notifier.notify_ready().is_ok());
    }

    #[test]
    fn test_systemd_notify_status() {
        let notifier = SystemdNotifier::new();
        assert!(notifier.notify_status("Running").is_ok());
    }

    #[test]
    fn test_shutdown_coordinator_creation() {
        let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));
        assert!(!coordinator.should_shutdown());
    }

    #[test]
    fn test_shutdown_coordinator_request() {
        let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));
        coordinator.request_shutdown();
        assert!(coordinator.should_shutdown());
    }

    #[test]
    fn test_shutdown_coordinator_timeout() {
        let coordinator = ShutdownCoordinator::new(Duration::from_secs(15));
        assert_eq!(coordinator.timeout(), Duration::from_secs(15));
    }

    #[test]
    fn test_shutdown_coordinator_default() {
        let coordinator = ShutdownCoordinator::default();
        assert_eq!(coordinator.timeout(), Duration::from_secs(30));
    }
}
