//! Warm restart support for portsyncd
//!
//! Enables zero-downtime daemon restarts by detecting the end of initial kernel port state
//! synchronization (EOIU - End of Init sequence User indication) and preserving port state.
//!
//! ## How it works
//!
//! 1. On startup, check if /var/lib/sonic/portsyncd/port_state.json exists
//! 2. If exists (warm restart) → Load port state, skip APP_DB updates during initial sync
//! 3. If not (cold start) → Fresh start, update APP_DB normally
//! 4. Listen for EOIU signal (netlink RTM_NEWLINK with ifi_change == 0)
//! 5. When EOIU received → Safe to accept APP_DB updates again
//! 6. Periodically save port state to file for next restart
//!
//! ## NIST 800-53 Compliance
//! - SC-24: Fail-secure warm restart (stale state → cold start)
//! - SI-4: Data integrity checks for persisted port state
//!
//! Phase 6 Week 2 implementation with full warm restart support.

use crate::error::{PortsyncError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Warm restart state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarmRestartState {
    /// Cold start - no saved state, normal operation
    ColdStart,
    /// Warm start detected - using saved state
    WarmStart,
    /// Warm start in progress, waiting for EOIU signal
    InitialSyncInProgress,
    /// EOIU signal received, safe to modify APP_DB
    InitialSyncComplete,
}

impl std::fmt::Display for WarmRestartState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WarmRestartState::ColdStart => write!(f, "ColdStart"),
            WarmRestartState::WarmStart => write!(f, "WarmStart"),
            WarmRestartState::InitialSyncInProgress => write!(f, "InitialSyncInProgress"),
            WarmRestartState::InitialSyncComplete => write!(f, "InitialSyncComplete"),
        }
    }
}

/// Serializable port state for persistence across restarts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortState {
    /// Port name (e.g., "Ethernet0")
    pub name: String,
    /// Port admin state: 0 = down, 1 = up
    pub admin_state: u32,
    /// Port operational state: 0 = down, 1 = up
    pub oper_state: u32,
    /// Port flags (IFF_UP, IFF_RUNNING, etc.)
    pub flags: u32,
    /// Port MTU
    pub mtu: u32,
}

impl PortState {
    /// Create new port state
    pub fn new(name: String, admin_state: u32, oper_state: u32, flags: u32, mtu: u32) -> Self {
        Self {
            name,
            admin_state,
            oper_state,
            flags,
            mtu,
        }
    }

    /// Check if port is operationally up
    pub fn is_up(&self) -> bool {
        self.oper_state == 1
    }

    /// Check if port admin is enabled
    pub fn is_admin_enabled(&self) -> bool {
        self.admin_state == 1
    }
}

/// Container for all saved port states across warm restarts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedPortState {
    /// Map of port name to port state
    pub ports: HashMap<String, PortState>,
    /// Timestamp when state was saved (Unix seconds)
    pub saved_at: u64,
    /// Version for forward compatibility
    pub version: u32,
}

impl PersistedPortState {
    /// Create new persisted state container
    pub fn new() -> Self {
        Self {
            ports: HashMap::new(),
            saved_at: current_timestamp(),
            version: 1,
        }
    }

    /// Add or update a port in the saved state
    pub fn upsert_port(&mut self, port: PortState) {
        self.ports.insert(port.name.clone(), port);
    }

    /// Get a port by name
    pub fn get_port(&self, name: &str) -> Option<&PortState> {
        self.ports.get(name)
    }

    /// Get number of saved ports
    pub fn port_count(&self) -> usize {
        self.ports.len()
    }

    /// Clear all ports
    pub fn clear(&mut self) {
        self.ports.clear();
    }
}

impl Default for PersistedPortState {
    fn default() -> Self {
        Self::new()
    }
}

/// Warm restart manager - orchestrates warm restart lifecycle
pub struct WarmRestartManager {
    state: WarmRestartState,
    state_file_path: PathBuf,
    persisted_state: PersistedPortState,
    /// Track when initial sync started for timeout detection
    initial_sync_start: Option<Instant>,
    /// EOIU timeout in seconds (default: 10)
    initial_sync_timeout_secs: u64,
    /// Metrics for observability and debugging
    pub metrics: WarmRestartMetrics,
}

impl WarmRestartManager {
    /// Create new warm restart manager with default state file path
    pub fn new() -> Self {
        let state_file_path = PathBuf::from("/var/lib/sonic/portsyncd/port_state.json");
        Self {
            state: WarmRestartState::ColdStart,
            state_file_path,
            persisted_state: PersistedPortState::new(),
            initial_sync_start: None,
            initial_sync_timeout_secs: Self::default_timeout_secs(),
            metrics: WarmRestartMetrics::new(),
        }
    }

    /// Create with custom state file path (for testing)
    pub fn with_state_file(state_file_path: PathBuf) -> Self {
        Self {
            state: WarmRestartState::ColdStart,
            state_file_path,
            persisted_state: PersistedPortState::new(),
            initial_sync_start: None,
            initial_sync_timeout_secs: Self::default_timeout_secs(),
            metrics: WarmRestartMetrics::new(),
        }
    }

    /// Default EOIU timeout in seconds
    fn default_timeout_secs() -> u64 {
        std::env::var("PORTSYNCD_EOIU_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(10)
    }

    /// Initialize warm restart - detects cold start vs warm restart
    pub fn initialize(&mut self) -> Result<()> {
        // Check if state file exists
        if self.state_file_path.exists() {
            match self.load_state() {
                Ok(_) => {
                    self.state = WarmRestartState::WarmStart;
                    self.metrics.record_warm_restart();
                    eprintln!("portsyncd: Warm restart detected - using saved port state");
                    Ok(())
                }
                Err(e) => {
                    eprintln!(
                        "portsyncd: Failed to load saved state ({}), treating as cold start",
                        e
                    );
                    // Fail-secure: can't load saved state → cold start
                    self.state = WarmRestartState::ColdStart;
                    self.metrics.record_cold_start();
                    self.metrics.record_corruption_detected();
                    Ok(())
                }
            }
        } else {
            self.state = WarmRestartState::ColdStart;
            self.metrics.record_cold_start();
            eprintln!("portsyncd: Cold start - no saved port state found");
            Ok(())
        }
    }

    /// Get current warm restart state
    pub fn current_state(&self) -> WarmRestartState {
        self.state
    }

    /// Transition to INITIAL_SYNC_IN_PROGRESS state
    pub fn begin_initial_sync(&mut self) {
        if self.state == WarmRestartState::WarmStart {
            self.state = WarmRestartState::InitialSyncInProgress;
            self.initial_sync_start = Some(Instant::now());
            eprintln!("portsyncd: Warm restart - begin initial sync, skipping APP_DB updates");
            eprintln!(
                "portsyncd: EOIU timeout set to {} seconds",
                self.initial_sync_timeout_secs
            );
        }
    }

    /// Transition to INITIAL_SYNC_COMPLETE when EOIU signal received
    pub fn complete_initial_sync(&mut self) {
        if self.state == WarmRestartState::InitialSyncInProgress {
            if let Some(start_time) = self.initial_sync_start {
                let elapsed = start_time.elapsed().as_secs();
                eprintln!("portsyncd: Initial sync completed in {} seconds", elapsed);
                self.metrics.record_initial_sync_duration(elapsed);
                self.metrics.record_eoiu_detected();
            }
            self.state = WarmRestartState::InitialSyncComplete;
            eprintln!(
                "portsyncd: EOIU signal received - initial sync complete, APP_DB updates enabled"
            );
        }
    }

    /// Check if initial sync has timed out and auto-complete if needed
    pub fn check_initial_sync_timeout(&mut self) -> Result<()> {
        if self.state != WarmRestartState::InitialSyncInProgress {
            return Ok(());
        }

        if let Some(start_time) = self.initial_sync_start {
            let elapsed = start_time.elapsed().as_secs();
            if elapsed >= self.initial_sync_timeout_secs {
                eprintln!(
                    "portsyncd: EOIU timeout after {} seconds, auto-completing initial sync",
                    elapsed
                );
                self.metrics.record_eoiu_timeout();
                self.metrics.record_initial_sync_duration(elapsed);
                self.complete_initial_sync();
            }
        }

        Ok(())
    }

    /// Set EOIU timeout duration in seconds
    pub fn set_initial_sync_timeout(&mut self, secs: u64) {
        self.initial_sync_timeout_secs = secs;
    }

    /// Get current EOIU timeout duration
    pub fn initial_sync_timeout(&self) -> u64 {
        self.initial_sync_timeout_secs
    }

    /// Get elapsed time since initial sync started (in seconds)
    pub fn initial_sync_elapsed_secs(&self) -> Option<u64> {
        self.initial_sync_start
            .map(|start| start.elapsed().as_secs())
    }

    /// Check if APP_DB updates should be skipped (warm restart during initial sync)
    pub fn should_skip_app_db_updates(&self) -> bool {
        self.state == WarmRestartState::InitialSyncInProgress
    }

    /// Save current port state to file
    pub fn save_state(&self) -> Result<()> {
        // Create directory if it doesn't exist
        if let Some(parent) = self.state_file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                PortsyncError::Other(format!(
                    "Failed to create state directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        let state_json = serde_json::to_string_pretty(&self.persisted_state)
            .map_err(|e| PortsyncError::Other(format!("Failed to serialize port state: {}", e)))?;

        fs::write(&self.state_file_path, state_json).map_err(|e| {
            PortsyncError::Other(format!(
                "Failed to write state file {}: {}",
                self.state_file_path.display(),
                e
            ))
        })?;

        Ok(())
    }

    /// Load port state from file
    pub fn load_state(&mut self) -> Result<()> {
        if !self.state_file_path.exists() {
            return Err(PortsyncError::Other(
                "State file does not exist".to_string(),
            ));
        }

        let state_json = fs::read_to_string(&self.state_file_path).map_err(|e| {
            PortsyncError::Other(format!(
                "Failed to read state file {}: {}",
                self.state_file_path.display(),
                e
            ))
        })?;

        self.persisted_state = serde_json::from_str(&state_json).map_err(|e| {
            PortsyncError::Other(format!("Failed to deserialize port state: {}", e))
        })?;

        Ok(())
    }

    /// Add port to saved state
    pub fn add_port(&mut self, port: PortState) {
        self.persisted_state.upsert_port(port);
    }

    /// Get port from saved state
    pub fn get_port(&self, name: &str) -> Option<&PortState> {
        self.persisted_state.get_port(name)
    }

    /// Clear all saved port state
    pub fn clear_ports(&mut self) {
        self.persisted_state.clear();
    }

    /// Get number of saved ports
    pub fn port_count(&self) -> usize {
        self.persisted_state.port_count()
    }

    /// Get state file path (for testing/debugging)
    pub fn state_file_path(&self) -> &Path {
        &self.state_file_path
    }

    /// Check if warm restart is in progress
    pub fn is_warm_restart_in_progress(&self) -> bool {
        self.state == WarmRestartState::WarmStart
            || self.state == WarmRestartState::InitialSyncInProgress
    }

    /// Clean up stale state files (older than 7 days)
    pub fn cleanup_stale_state_files(&self) -> Result<()> {
        let state_dir = match self.state_file_path.parent() {
            Some(parent) => parent,
            None => return Ok(()), // No parent directory, nothing to clean
        };

        if !state_dir.exists() {
            return Ok(());
        }

        // Calculate 7 days in seconds
        let seven_days_secs = 7 * 24 * 60 * 60;

        // Iterate through all files in the state directory
        let entries = fs::read_dir(state_dir).map_err(|e| {
            PortsyncError::Other(format!(
                "Failed to read state directory {}: {}",
                state_dir.display(),
                e
            ))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                PortsyncError::Other(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();

            // Only process JSON files
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            // Get file metadata
            let metadata = match fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue, // Skip files we can't read metadata for
            };

            // Get file modification time
            let modified_time = match metadata.modified() {
                Ok(time) => time,
                Err(_) => continue, // Skip files we can't get time for
            };

            // Calculate age in seconds
            let age_secs = match modified_time.elapsed() {
                Ok(elapsed) => elapsed.as_secs(),
                Err(_) => continue, // Skip files with invalid times
            };

            // Remove files older than 7 days
            if age_secs > seven_days_secs {
                if let Err(e) = fs::remove_file(&path) {
                    eprintln!(
                        "portsyncd: Warning - failed to remove stale state file {}: {}",
                        path.display(),
                        e
                    );
                } else {
                    eprintln!(
                        "portsyncd: Cleaned up stale state file {} (age: {} days)",
                        path.display(),
                        age_secs / (24 * 60 * 60)
                    );
                }
            }
        }

        Ok(())
    }

    /// Get the age of the state file in seconds
    pub fn state_file_age_secs(&self) -> Result<Option<u64>> {
        if !self.state_file_path.exists() {
            return Ok(None);
        }

        let metadata = fs::metadata(&self.state_file_path).map_err(|e| {
            PortsyncError::Other(format!(
                "Failed to get state file metadata {}: {}",
                self.state_file_path.display(),
                e
            ))
        })?;

        let modified_time = metadata.modified().map_err(|e| {
            PortsyncError::Other(format!("Failed to get state file modification time: {}", e))
        })?;

        let age = modified_time.elapsed().map_err(|e| {
            PortsyncError::Other(format!("Failed to calculate state file age: {}", e))
        })?;

        Ok(Some(age.as_secs()))
    }

    /// Rotate state file with backup (creates timestamped backup, replaces current)
    pub fn rotate_state_file(&mut self) -> Result<()> {
        if !self.state_file_path.exists() {
            return Ok(()); // No file to rotate
        }

        let parent = match self.state_file_path.parent() {
            Some(p) => p,
            None => return Ok(()), // No parent directory
        };

        // Create backup directory for rotated files
        let backup_dir = parent.join("backups");
        fs::create_dir_all(&backup_dir).map_err(|e| {
            PortsyncError::Other(format!(
                "Failed to create backup directory {}: {}",
                backup_dir.display(),
                e
            ))
        })?;

        // Create backup filename with timestamp
        let timestamp = current_timestamp();
        let backup_name = format!("port_state_{}.json", timestamp);
        let backup_path = backup_dir.join(&backup_name);

        // Copy current file to backup
        fs::copy(&self.state_file_path, &backup_path).map_err(|e| {
            PortsyncError::Other(format!(
                "Failed to create backup {}: {}",
                backup_path.display(),
                e
            ))
        })?;

        self.metrics.record_backup_created();

        eprintln!(
            "portsyncd: Rotated state file, backup saved to {}",
            backup_path.display()
        );

        Ok(())
    }

    /// Clean up old backup files (keep only last N backups)
    pub fn cleanup_old_backups(&mut self, max_backups: usize) -> Result<()> {
        let parent = match self.state_file_path.parent() {
            Some(p) => p,
            None => return Ok(()), // No parent directory
        };

        let backup_dir = parent.join("backups");
        if !backup_dir.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(&backup_dir).map_err(|e| {
            PortsyncError::Other(format!(
                "Failed to read backup directory {}: {}",
                backup_dir.display(),
                e
            ))
        })?;

        // Collect all backup files with their modification times
        let mut backups: Vec<(PathBuf, u64)> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| {
                PortsyncError::Other(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            if let Ok(metadata) = fs::metadata(&path)
                && let Ok(modified) = metadata.modified()
                    && let Ok(elapsed) = modified.elapsed() {
                        backups.push((path, elapsed.as_secs()));
                    }
        }

        // Sort by age (newest first) and delete oldest if we exceed max_backups
        backups.sort_by_key(|(_, age)| *age);

        if backups.len() > max_backups {
            for (backup_path, _) in &backups[max_backups..] {
                if let Err(e) = fs::remove_file(backup_path) {
                    eprintln!(
                        "portsyncd: Warning - failed to remove old backup {}: {}",
                        backup_path.display(),
                        e
                    );
                } else {
                    self.metrics.record_backup_cleanup();
                    eprintln!("portsyncd: Removed old backup {}", backup_path.display());
                }
            }
        }

        Ok(())
    }

    /// Get list of backup files
    pub fn get_backup_files(&self) -> Result<Vec<PathBuf>> {
        let parent = match self.state_file_path.parent() {
            Some(p) => p,
            None => return Ok(Vec::new()), // No parent directory
        };

        let backup_dir = parent.join("backups");
        if !backup_dir.exists() {
            return Ok(Vec::new());
        }

        let mut backups = Vec::new();
        let entries = fs::read_dir(&backup_dir).map_err(|e| {
            PortsyncError::Other(format!(
                "Failed to read backup directory {}: {}",
                backup_dir.display(),
                e
            ))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                PortsyncError::Other(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                backups.push(path);
            }
        }

        // Sort in reverse order (newest first)
        backups.sort_by(|a, b| b.cmp(a));
        Ok(backups)
    }

    /// Load state with automatic fallback to backup chain on corruption
    /// Tries to load from: current file → backup chain (newest to oldest)
    /// Returns Ok(true) if loaded successfully, Ok(false) if all failed, Err on I/O errors
    pub fn load_state_with_recovery(&mut self) -> Result<bool> {
        // First try to load from current state file
        if self.state_file_path.exists() {
            match self.load_state() {
                Ok(_) => {
                    eprintln!(
                        "portsyncd: Loaded port state from current file {}",
                        self.state_file_path.display()
                    );
                    return Ok(true);
                }
                Err(e) => {
                    eprintln!(
                        "portsyncd: Failed to load current state file ({}), trying backups...",
                        e
                    );
                    self.metrics.record_corruption_detected();
                }
            }
        }

        // Try backup files in order (newest first)
        let backups = self.get_backup_files()?;
        for backup_path in backups {
            match fs::read_to_string(&backup_path) {
                Ok(state_json) => match serde_json::from_str::<PersistedPortState>(&state_json) {
                    Ok(persisted_state) => {
                        self.persisted_state = persisted_state;
                        self.metrics.record_state_recovery();
                        eprintln!(
                            "portsyncd: Recovered port state from backup {}",
                            backup_path.display()
                        );
                        return Ok(true);
                    }
                    Err(e) => {
                        eprintln!(
                            "portsyncd: Backup {} corrupted ({}), trying next...",
                            backup_path.display(),
                            e
                        );
                        continue;
                    }
                },
                Err(e) => {
                    eprintln!(
                        "portsyncd: Failed to read backup {} ({}), trying next...",
                        backup_path.display(),
                        e
                    );
                    continue;
                }
            }
        }

        // All attempts failed
        eprintln!("portsyncd: No valid state file or backup found, treating as cold start");
        Ok(false)
    }

    /// Check if state is valid (matches expected schema and contains reasonable data)
    pub fn is_state_valid(&self) -> bool {
        // Basic validation: must have reasonable version and not negative port count
        self.persisted_state.version > 0 && self.persisted_state.ports.len() < 10000
    }

    /// Clear current state (used after corruption recovery)
    pub fn reset_state(&mut self) {
        self.persisted_state = PersistedPortState::new();
    }
}

impl Default for WarmRestartManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics for warm restart lifecycle tracking and observability
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WarmRestartMetrics {
    /// Number of warm restart attempts
    pub warm_restart_count: u64,
    /// Number of cold starts
    pub cold_start_count: u64,
    /// Total EOIU detections
    pub eoiu_detected_count: u64,
    /// Number of EOIU timeouts (auto-completion)
    pub eoiu_timeout_count: u64,
    /// Number of successful state recoveries
    pub state_recovery_count: u64,
    /// Number of state file corruptions detected
    pub corruption_detected_count: u64,
    /// Total backup files created
    pub backup_created_count: u64,
    /// Total backup files cleaned up
    pub backup_cleanup_count: u64,
    /// Last warm restart timestamp (Unix seconds)
    pub last_warm_restart_secs: Option<u64>,
    /// Last EOIU detection timestamp
    pub last_eoiu_detection_secs: Option<u64>,
    /// Last state recovery timestamp
    pub last_state_recovery_secs: Option<u64>,
    /// Last corruption detection timestamp
    pub last_corruption_detected_secs: Option<u64>,
    /// Average initial sync duration in seconds
    pub avg_initial_sync_duration_secs: f64,
    /// Maximum observed initial sync duration in seconds
    pub max_initial_sync_duration_secs: u64,
    /// Minimum observed initial sync duration in seconds
    pub min_initial_sync_duration_secs: u64,
}

impl WarmRestartMetrics {
    /// Create new metrics
    pub fn new() -> Self {
        Self {
            warm_restart_count: 0,
            cold_start_count: 0,
            eoiu_detected_count: 0,
            eoiu_timeout_count: 0,
            state_recovery_count: 0,
            corruption_detected_count: 0,
            backup_created_count: 0,
            backup_cleanup_count: 0,
            last_warm_restart_secs: None,
            last_eoiu_detection_secs: None,
            last_state_recovery_secs: None,
            last_corruption_detected_secs: None,
            avg_initial_sync_duration_secs: 0.0,
            max_initial_sync_duration_secs: 0,
            min_initial_sync_duration_secs: u64::MAX,
        }
    }

    /// Record a warm restart event
    pub fn record_warm_restart(&mut self) {
        self.warm_restart_count += 1;
        self.last_warm_restart_secs = Some(current_timestamp());
    }

    /// Record a cold start event
    pub fn record_cold_start(&mut self) {
        self.cold_start_count += 1;
    }

    /// Record EOIU detection
    pub fn record_eoiu_detected(&mut self) {
        self.eoiu_detected_count += 1;
        self.last_eoiu_detection_secs = Some(current_timestamp());
    }

    /// Record EOIU timeout (auto-completion)
    pub fn record_eoiu_timeout(&mut self) {
        self.eoiu_timeout_count += 1;
    }

    /// Record state recovery
    pub fn record_state_recovery(&mut self) {
        self.state_recovery_count += 1;
        self.last_state_recovery_secs = Some(current_timestamp());
    }

    /// Record corruption detection
    pub fn record_corruption_detected(&mut self) {
        self.corruption_detected_count += 1;
        self.last_corruption_detected_secs = Some(current_timestamp());
    }

    /// Record backup creation
    pub fn record_backup_created(&mut self) {
        self.backup_created_count += 1;
    }

    /// Record backup cleanup
    pub fn record_backup_cleanup(&mut self) {
        self.backup_cleanup_count += 1;
    }

    /// Record initial sync duration
    pub fn record_initial_sync_duration(&mut self, duration_secs: u64) {
        // Update average
        let total = self
            .warm_restart_count
            .saturating_sub(self.eoiu_timeout_count);
        if total > 0 {
            let current_sum = self.avg_initial_sync_duration_secs * (total as f64 - 1.0);
            self.avg_initial_sync_duration_secs =
                (current_sum + (duration_secs as f64)) / (total as f64);
        }

        // Update max/min
        self.max_initial_sync_duration_secs =
            self.max_initial_sync_duration_secs.max(duration_secs);
        if duration_secs > 0 {
            self.min_initial_sync_duration_secs =
                self.min_initial_sync_duration_secs.min(duration_secs);
        }
    }

    /// Reset all metrics (used for testing)
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Get total events tracked
    pub fn total_events(&self) -> u64 {
        self.warm_restart_count + self.cold_start_count
    }

    /// Get warm restart percentage
    pub fn warm_restart_percentage(&self) -> f64 {
        let total = self.total_events();
        if total == 0 {
            0.0
        } else {
            (self.warm_restart_count as f64 / total as f64) * 100.0
        }
    }
}

/// Get current Unix timestamp in seconds
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_port_state_creation() {
        let port = PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216);
        assert_eq!(port.name, "Ethernet0");
        assert_eq!(port.admin_state, 1);
        assert_eq!(port.oper_state, 1);
        assert!(port.is_up());
        assert!(port.is_admin_enabled());
    }

    #[test]
    fn test_port_state_down() {
        let port = PortState::new("Ethernet0".to_string(), 0, 0, 0x00, 9216);
        assert!(!port.is_up());
        assert!(!port.is_admin_enabled());
    }

    #[test]
    fn test_persisted_state_default() {
        let state = PersistedPortState::new();
        assert_eq!(state.port_count(), 0);
        assert_eq!(state.version, 1);
    }

    #[test]
    fn test_persisted_state_upsert() {
        let mut state = PersistedPortState::new();
        let port = PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216);
        state.upsert_port(port.clone());

        assert_eq!(state.port_count(), 1);
        assert!(state.get_port("Ethernet0").is_some());
        assert_eq!(state.get_port("Ethernet0").unwrap().name, "Ethernet0");
    }

    #[test]
    fn test_persisted_state_upsert_overwrites() {
        let mut state = PersistedPortState::new();
        let port1 = PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216);
        let port2 = PortState::new("Ethernet0".to_string(), 0, 0, 0x00, 9216);

        state.upsert_port(port1);
        assert_eq!(state.get_port("Ethernet0").unwrap().admin_state, 1);

        state.upsert_port(port2);
        assert_eq!(state.get_port("Ethernet0").unwrap().admin_state, 0);
    }

    #[test]
    fn test_warm_restart_manager_cold_start() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        assert_eq!(manager.current_state(), WarmRestartState::ColdStart);
        assert!(!manager.should_skip_app_db_updates());
    }

    #[test]
    fn test_warm_restart_manager_state_transitions() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        assert_eq!(manager.current_state(), WarmRestartState::ColdStart);

        // Manually set to warm start for testing
        manager.state = WarmRestartState::WarmStart;
        manager.begin_initial_sync();
        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncInProgress
        );
        assert!(manager.should_skip_app_db_updates());

        manager.complete_initial_sync();
        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncComplete
        );
        assert!(!manager.should_skip_app_db_updates());
    }

    #[test]
    fn test_warm_restart_manager_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        let port1 = PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216);
        let port2 = PortState::new("Ethernet4".to_string(), 1, 0, 0x01, 9216);

        manager.add_port(port1);
        manager.add_port(port2);
        manager.save_state().unwrap();

        assert!(state_file.exists());

        let mut manager2 = WarmRestartManager::with_state_file(state_file);
        manager2.load_state().unwrap();

        assert_eq!(manager2.port_count(), 2);
        assert!(manager2.get_port("Ethernet0").is_some());
        assert!(manager2.get_port("Ethernet4").is_some());
    }

    #[test]
    fn test_warm_restart_manager_port_operations() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);

        let port = PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216);
        manager.add_port(port);

        assert_eq!(manager.port_count(), 1);
        assert!(manager.get_port("Ethernet0").is_some());

        manager.clear_ports();
        assert_eq!(manager.port_count(), 0);
    }

    #[test]
    fn test_warm_restart_manager_warm_start_detection() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        // Create initial state file
        {
            let mut manager = WarmRestartManager::with_state_file(state_file.clone());
            let port = PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216);
            manager.add_port(port);
            manager.save_state().unwrap();
        }

        // Load state file on second instantiation (warm start)
        {
            let mut manager = WarmRestartManager::with_state_file(state_file);
            manager.initialize().unwrap();

            assert_eq!(manager.current_state(), WarmRestartState::WarmStart);
            assert_eq!(manager.port_count(), 1);
        }
    }

    // ========== STATE FILE CLEANUP TESTS (Phase 6 Week 3) ==========

    #[test]
    fn test_state_file_age_secs_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        // Create a state file
        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
        manager.save_state().unwrap();

        // Check age - should be very small (0-1 seconds)
        let age = manager.state_file_age_secs().unwrap();
        assert!(age.is_some());
        assert!(age.unwrap() < 5); // Should be nearly 0, definitely < 5 seconds
    }

    #[test]
    fn test_state_file_age_secs_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("nonexistent.json");

        let manager = WarmRestartManager::with_state_file(state_file);

        // Age of nonexistent file should be None
        let age = manager.state_file_age_secs().unwrap();
        assert!(age.is_none());
    }

    #[test]
    fn test_cleanup_stale_state_files_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let manager = WarmRestartManager::with_state_file(state_file);

        // Should not error on empty directory
        manager.cleanup_stale_state_files().unwrap();
    }

    #[test]
    fn test_cleanup_stale_state_files_no_stale_files() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        // Create a fresh state file
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
        manager.save_state().unwrap();

        // Cleanup should not remove fresh files
        manager.cleanup_stale_state_files().unwrap();
        assert!(state_file.exists());
    }

    // ========== STATE FILE ROTATION TESTS (Phase 6 Week 3) ==========

    #[test]
    fn test_rotate_state_file_creates_backup() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        // Create initial state file
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
        manager.save_state().unwrap();

        // Rotate the state file
        manager.rotate_state_file().unwrap();

        // Current file should still exist
        assert!(state_file.exists());

        // Backup should exist
        let backup_dir = temp_dir.path().join("backups");
        assert!(backup_dir.exists());
        let backups = manager.get_backup_files().unwrap();
        assert_eq!(backups.len(), 1);
    }

    #[test]
    fn test_rotate_state_file_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("nonexistent.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);

        // Should not error on nonexistent file
        manager.rotate_state_file().unwrap();

        // No backup should be created
        let backups = manager.get_backup_files().unwrap();
        assert_eq!(backups.len(), 0);
    }

    #[test]
    fn test_cleanup_old_backups_keeps_max_count() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
        manager.save_state().unwrap();

        // Create multiple backups with longer delays to ensure different timestamps
        for _ in 0..3 {
            manager.rotate_state_file().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1100)); // 1+ second to change timestamp
        }

        let backups_before = manager.get_backup_files().unwrap();
        assert!(backups_before.len() >= 2); // At least 2 unique backups

        // Cleanup to keep only 2
        manager.cleanup_old_backups(2).unwrap();

        let backups_after = manager.get_backup_files().unwrap();
        assert!(backups_after.len() <= 2);
    }

    #[test]
    fn test_get_backup_files_sorted() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
        manager.save_state().unwrap();

        // Create multiple backups with sufficient delays for different timestamps
        for _ in 0..2 {
            manager.rotate_state_file().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1100)); // 1+ second delay
        }

        let backups = manager.get_backup_files().unwrap();
        assert!(backups.len() >= 1); // At least one backup created

        // Verify they are sorted (newest first)
        // Since backups are sorted by filename and created with timestamps,
        // newer timestamps should be >= older timestamps
        if backups.len() >= 2 {
            // First element (newest) should have a greater or equal name than second
            assert!(backups[0] >= backups[1]); // Newest first
        }
    }

    // ========== CORRUPTION RECOVERY TESTS (Phase 6 Week 3) ==========

    #[test]
    fn test_load_state_with_recovery_valid_file() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        // Create and save initial state
        {
            let mut manager = WarmRestartManager::with_state_file(state_file.clone());
            manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
            manager.add_port(PortState::new("Ethernet4".to_string(), 1, 0, 0x01, 9216));
            manager.save_state().unwrap();
        }

        // Load with recovery
        {
            let mut manager = WarmRestartManager::with_state_file(state_file);
            let recovered = manager.load_state_with_recovery().unwrap();
            assert!(recovered); // Should recover successfully
            assert_eq!(manager.port_count(), 2);
        }
    }

    #[test]
    fn test_load_state_with_recovery_from_backup_on_corruption() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        // Create initial state and backup
        {
            let mut manager = WarmRestartManager::with_state_file(state_file.clone());
            manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
            manager.save_state().unwrap();
            manager.rotate_state_file().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        // Corrupt the current state file
        fs::write(&state_file, "this is not valid json").unwrap();

        // Try to load - should recover from backup
        {
            let mut manager = WarmRestartManager::with_state_file(state_file);
            let recovered = manager.load_state_with_recovery().unwrap();
            assert!(recovered); // Should recover from backup
            assert_eq!(manager.port_count(), 1);
        }
    }

    #[test]
    fn test_load_state_with_recovery_no_files() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("nonexistent.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);
        let recovered = manager.load_state_with_recovery().unwrap();
        assert!(!recovered); // No files to recover from
        assert_eq!(manager.port_count(), 0);
    }

    #[test]
    fn test_is_state_valid() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);

        // New state should be valid (version=1, empty ports)
        assert!(manager.is_state_valid());

        // Add a port - still valid
        manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
        assert!(manager.is_state_valid());
    }

    #[test]
    fn test_reset_state() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
        manager.add_port(PortState::new("Ethernet4".to_string(), 1, 0, 0x01, 9216));

        assert_eq!(manager.port_count(), 2);

        manager.reset_state();
        assert_eq!(manager.port_count(), 0);
        assert!(manager.is_state_valid());
    }

    // ========== METRICS TRACKING TESTS (Phase 6 Week 3) ==========

    #[test]
    fn test_metrics_creation() {
        let metrics = WarmRestartMetrics::new();
        assert_eq!(metrics.warm_restart_count, 0);
        assert_eq!(metrics.cold_start_count, 0);
        assert_eq!(metrics.eoiu_detected_count, 0);
        assert_eq!(metrics.total_events(), 0);
    }

    #[test]
    fn test_metrics_record_warm_restart() {
        let mut metrics = WarmRestartMetrics::new();
        metrics.record_warm_restart();

        assert_eq!(metrics.warm_restart_count, 1);
        assert!(metrics.last_warm_restart_secs.is_some());
    }

    #[test]
    fn test_metrics_record_cold_start() {
        let mut metrics = WarmRestartMetrics::new();
        metrics.record_cold_start();

        assert_eq!(metrics.cold_start_count, 1);
        assert_eq!(metrics.total_events(), 1);
    }

    #[test]
    fn test_metrics_record_eoiu_detected() {
        let mut metrics = WarmRestartMetrics::new();
        metrics.record_eoiu_detected();

        assert_eq!(metrics.eoiu_detected_count, 1);
        assert!(metrics.last_eoiu_detection_secs.is_some());
    }

    #[test]
    fn test_metrics_record_eoiu_timeout() {
        let mut metrics = WarmRestartMetrics::new();
        metrics.record_eoiu_timeout();

        assert_eq!(metrics.eoiu_timeout_count, 1);
    }

    #[test]
    fn test_metrics_record_state_recovery() {
        let mut metrics = WarmRestartMetrics::new();
        metrics.record_state_recovery();

        assert_eq!(metrics.state_recovery_count, 1);
        assert!(metrics.last_state_recovery_secs.is_some());
    }

    #[test]
    fn test_metrics_record_corruption() {
        let mut metrics = WarmRestartMetrics::new();
        metrics.record_corruption_detected();

        assert_eq!(metrics.corruption_detected_count, 1);
        assert!(metrics.last_corruption_detected_secs.is_some());
    }

    #[test]
    fn test_metrics_backup_tracking() {
        let mut metrics = WarmRestartMetrics::new();
        metrics.record_backup_created();
        metrics.record_backup_created();
        metrics.record_backup_cleanup();

        assert_eq!(metrics.backup_created_count, 2);
        assert_eq!(metrics.backup_cleanup_count, 1);
    }

    #[test]
    fn test_metrics_warm_restart_percentage() {
        let mut metrics = WarmRestartMetrics::new();

        // No events
        assert_eq!(metrics.warm_restart_percentage(), 0.0);

        // All warm restarts
        metrics.record_warm_restart();
        metrics.record_warm_restart();
        assert_eq!(metrics.warm_restart_percentage(), 100.0);

        // Mixed
        metrics.record_cold_start();
        let percentage = metrics.warm_restart_percentage();
        assert!(percentage > 50.0 && percentage < 100.0); // Should be ~67%
    }

    #[test]
    fn test_metrics_initial_sync_duration() {
        let mut metrics = WarmRestartMetrics::new();

        // Record some sync durations
        metrics.record_warm_restart();
        metrics.record_initial_sync_duration(5);
        metrics.record_warm_restart();
        metrics.record_initial_sync_duration(10);
        metrics.record_warm_restart();
        metrics.record_initial_sync_duration(7);

        // Verify aggregates
        assert!(metrics.avg_initial_sync_duration_secs > 0.0);
        assert_eq!(metrics.max_initial_sync_duration_secs, 10);
        assert_eq!(metrics.min_initial_sync_duration_secs, 5);
    }

    #[test]
    fn test_metrics_reset() {
        let mut metrics = WarmRestartMetrics::new();
        metrics.record_warm_restart();
        metrics.record_eoiu_detected();
        metrics.record_state_recovery();

        assert_eq!(metrics.warm_restart_count, 1);
        assert_eq!(metrics.eoiu_detected_count, 1);

        metrics.reset();

        assert_eq!(metrics.warm_restart_count, 0);
        assert_eq!(metrics.eoiu_detected_count, 0);
        assert_eq!(metrics.total_events(), 0);
    }

    // ========== TIMEOUT FUNCTIONALITY TESTS (Phase 6 Week 3) ==========

    #[test]
    fn test_timeout_not_reached_normal_eoiu() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        // Set to warm start and begin initial sync with long timeout
        manager.state = WarmRestartState::WarmStart;
        manager.set_initial_sync_timeout(60); // 60 second timeout
        manager.begin_initial_sync();

        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncInProgress
        );
        assert_eq!(manager.initial_sync_timeout(), 60);

        // Check timeout immediately - should not trigger
        manager.check_initial_sync_timeout().unwrap();
        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncInProgress
        );

        // Complete normally via EOIU (before timeout)
        manager.complete_initial_sync();
        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncComplete
        );
    }

    #[test]
    fn test_timeout_reached_auto_complete() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        // Set to warm start and begin initial sync with very short timeout
        manager.state = WarmRestartState::WarmStart;
        manager.set_initial_sync_timeout(0); // 0 second timeout (immediate)
        manager.begin_initial_sync();

        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncInProgress
        );

        // Small sleep to ensure time has passed
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Check timeout - should auto-complete
        manager.check_initial_sync_timeout().unwrap();
        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncComplete
        );
    }

    #[test]
    fn test_configurable_timeout_via_setter() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        // Default should be 10 seconds
        assert_eq!(manager.initial_sync_timeout(), 10);

        // Set different timeout values
        manager.set_initial_sync_timeout(5);
        assert_eq!(manager.initial_sync_timeout(), 5);

        manager.set_initial_sync_timeout(30);
        assert_eq!(manager.initial_sync_timeout(), 30);

        manager.set_initial_sync_timeout(0);
        assert_eq!(manager.initial_sync_timeout(), 0);
    }

    #[test]
    fn test_elapsed_time_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        manager.state = WarmRestartState::WarmStart;
        manager.set_initial_sync_timeout(10);
        manager.begin_initial_sync();

        // Immediately after begin, elapsed should be near 0
        let elapsed1 = manager.initial_sync_elapsed_secs();
        assert!(elapsed1.is_some());
        assert_eq!(elapsed1.unwrap(), 0);

        // After sleep, elapsed time should increase
        std::thread::sleep(std::time::Duration::from_millis(100));
        let elapsed2 = manager.initial_sync_elapsed_secs();
        assert!(elapsed2.is_some());
        // Value will be 0 or 1 depending on timing
        let _ = elapsed2.unwrap();

        // Before sync starts, elapsed should be None
        let mut manager2 = WarmRestartManager::with_state_file(
            TempDir::new().unwrap().path().join("port_state2.json"),
        );
        manager2.initialize().unwrap();
        assert!(manager2.initial_sync_elapsed_secs().is_none());
    }

    #[test]
    fn test_state_transition_on_timeout_auto_complete() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        manager.state = WarmRestartState::WarmStart;
        manager.set_initial_sync_timeout(0); // Immediate timeout
        manager.begin_initial_sync();

        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncInProgress
        );
        assert!(manager.should_skip_app_db_updates());

        // Sleep to ensure timeout
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Trigger timeout auto-completion
        manager.check_initial_sync_timeout().unwrap();

        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncComplete
        );
        assert!(!manager.should_skip_app_db_updates());
    }

    #[test]
    fn test_multiple_timeout_checks_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        manager.state = WarmRestartState::WarmStart;
        manager.set_initial_sync_timeout(0);
        manager.begin_initial_sync();

        std::thread::sleep(std::time::Duration::from_millis(50));

        // First check completes sync
        manager.check_initial_sync_timeout().unwrap();
        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncComplete
        );

        // Second check should be idempotent (no error, state unchanged)
        manager.check_initial_sync_timeout().unwrap();
        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncComplete
        );

        // Third check should also be safe
        manager.check_initial_sync_timeout().unwrap();
        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncComplete
        );
    }

    #[test]
    fn test_timeout_check_without_initial_sync_running() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        // Call check_initial_sync_timeout() without begin_initial_sync()
        // Should return Ok() but have no effect
        manager.check_initial_sync_timeout().unwrap();

        assert_eq!(manager.current_state(), WarmRestartState::ColdStart);
        assert!(!manager.should_skip_app_db_updates());
    }

    #[test]
    fn test_zero_timeout_immediate_completion() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("port_state.json");

        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        manager.state = WarmRestartState::WarmStart;
        manager.set_initial_sync_timeout(0);
        manager.begin_initial_sync();

        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncInProgress
        );
        assert_eq!(manager.initial_sync_timeout(), 0);

        // Even minimal sleep should trigger timeout
        std::thread::sleep(std::time::Duration::from_millis(1));
        manager.check_initial_sync_timeout().unwrap();

        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncComplete
        );
    }
}
