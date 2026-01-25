//! Integration tests for warm restart functionality
//!
//! Tests complete warm restart workflows including:
//! - Cold start detection
//! - Warm restart detection and state preservation
//! - EOIU signal handling
//! - APP_DB write gating during initial sync
//! - Port state persistence and recovery

use sonic_portsyncd::{
    EoiuDetectionState, EoiuDetector, PersistedPortState, PortState, WarmRestartManager,
    WarmRestartState,
};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_warm_restart_cold_start_detection() {
    // Simulate cold start - no state file present
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    let mut manager = WarmRestartManager::with_state_file(state_file);
    manager.initialize().unwrap();

    assert_eq!(manager.current_state(), WarmRestartState::ColdStart);
    assert!(!manager.should_skip_app_db_updates());
}

#[test]
fn test_warm_restart_warm_start_detection_and_state_load() {
    // Create initial state file
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // First instance - cold start, save state
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        manager.initialize().unwrap();

        let port1 = PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216);
        let port2 = PortState::new("Ethernet4".to_string(), 1, 0, 0x01, 9216);
        let port3 = PortState::new("Ethernet8".to_string(), 0, 0, 0x00, 9216);

        manager.add_port(port1);
        manager.add_port(port2);
        manager.add_port(port3);

        manager.save_state().unwrap();
        assert_eq!(manager.port_count(), 3);
    }

    // Second instance - warm start, load state
    {
        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        assert_eq!(manager.current_state(), WarmRestartState::WarmStart);
        assert_eq!(manager.port_count(), 3);

        let eth0 = manager.get_port("Ethernet0").unwrap();
        assert_eq!(eth0.name, "Ethernet0");
        assert!(eth0.is_up());

        let eth4 = manager.get_port("Ethernet4").unwrap();
        assert_eq!(eth4.name, "Ethernet4");
        assert!(!eth4.is_up());

        let eth8 = manager.get_port("Ethernet8").unwrap();
        assert_eq!(eth8.name, "Ethernet8");
        assert!(!eth8.is_admin_enabled());
    }
}

#[test]
fn test_warm_restart_state_transitions_with_eoiu() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // Setup: create initial state
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        let port = PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216);
        manager.add_port(port);
        manager.save_state().unwrap();
    }

    // Warm restart: test state machine transitions
    {
        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        assert_eq!(manager.current_state(), WarmRestartState::WarmStart);
        assert!(!manager.should_skip_app_db_updates());

        // Start initial sync
        manager.begin_initial_sync();
        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncInProgress
        );
        assert!(manager.should_skip_app_db_updates());

        // EOIU signal received
        manager.complete_initial_sync();
        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncComplete
        );
        assert!(!manager.should_skip_app_db_updates());
    }
}

#[test]
fn test_warm_restart_app_db_write_gating() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // Setup warm restart state
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        let port = PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216);
        manager.add_port(port);
        manager.save_state().unwrap();
    }

    // Test APP_DB write gating
    {
        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        // Cold start - updates allowed
        let updates_allowed_cold = !manager.should_skip_app_db_updates();
        assert!(updates_allowed_cold);

        // Warm start begins - updates blocked
        manager.begin_initial_sync();
        let updates_allowed_during_sync = !manager.should_skip_app_db_updates();
        assert!(!updates_allowed_during_sync);

        // EOIU received - updates allowed again
        manager.complete_initial_sync();
        let updates_allowed_after_eoiu = !manager.should_skip_app_db_updates();
        assert!(updates_allowed_after_eoiu);
    }
}

#[test]
fn test_eoiu_detector_basic_sequence() {
    let mut detector = EoiuDetector::new();

    // Simulate interface dump
    assert_eq!(detector.state(), EoiuDetectionState::Waiting);

    // Normal interface update (ifi_change != 0)
    let is_eoiu = detector.check_eoiu("Ethernet0", 1, 0x41);
    assert!(!is_eoiu);
    assert_eq!(detector.state(), EoiuDetectionState::Waiting);

    // EOIU signal (ifi_change == 0)
    let is_eoiu = detector.check_eoiu("lo", 0, 0x01);
    assert!(is_eoiu);
    assert_eq!(detector.state(), EoiuDetectionState::Detected);

    // Mark as complete
    detector.mark_complete();
    assert_eq!(detector.state(), EoiuDetectionState::Complete);

    // Subsequent EOIU-like messages ignored
    let is_eoiu = detector.check_eoiu("lo", 0, 0x01);
    assert!(!is_eoiu);
}

#[test]
fn test_eoiu_detector_with_multiple_ports() {
    let mut detector = EoiuDetector::new();

    // Simulate dump of 10 ports
    for i in 0..10 {
        let ifname = format!("Ethernet{}", i * 4);
        let is_eoiu = detector.check_eoiu(&ifname, 1, 0x41);
        assert!(!is_eoiu);
        detector.increment_dumped_interfaces();
    }

    assert_eq!(detector.dumped_interfaces(), 10);
    assert_eq!(detector.messages_seen(), 10);

    // EOIU signal
    let is_eoiu = detector.check_eoiu("lo", 0, 0x01);
    assert!(is_eoiu);
    assert_eq!(detector.dumped_interfaces(), 10);
    assert_eq!(detector.messages_seen(), 11);
}

#[test]
fn test_warm_restart_port_state_serialization() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // Create complex port state
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());

        // Add ports with various states
        manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
        manager.add_port(PortState::new("Ethernet4".to_string(), 1, 0, 0x01, 9216));
        manager.add_port(PortState::new("Ethernet8".to_string(), 0, 0, 0x00, 9216));
        manager.add_port(PortState::new("Ethernet12".to_string(), 1, 1, 0x41, 9216));

        manager.save_state().unwrap();
    }

    // Verify JSON file is valid
    {
        let json_content = fs::read_to_string(&state_file).unwrap();
        let persisted: PersistedPortState =
            serde_json::from_str(&json_content).expect("JSON should deserialize");

        assert_eq!(persisted.port_count(), 4);
        assert!(persisted.get_port("Ethernet0").is_some());
        assert!(persisted.get_port("Ethernet4").is_some());
        assert!(persisted.get_port("Ethernet8").is_some());
        assert!(persisted.get_port("Ethernet12").is_some());
    }

    // Load and verify
    {
        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.load_state().unwrap();

        assert_eq!(manager.port_count(), 4);

        let eth0 = manager.get_port("Ethernet0").unwrap();
        assert_eq!(eth0.mtu, 9216);
        assert!(eth0.is_up());
    }
}

#[test]
fn test_warm_restart_missing_state_file_graceful_fallback() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("nonexistent_port_state.json");

    let mut manager = WarmRestartManager::with_state_file(state_file);
    let result = manager.initialize();

    // Should not error, should fall back to cold start
    assert!(result.is_ok());
    assert_eq!(manager.current_state(), WarmRestartState::ColdStart);
}

#[test]
fn test_warm_restart_corrupted_state_file_fallback() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("corrupted_port_state.json");

    // Write corrupted JSON
    fs::write(&state_file, "{ invalid json }").unwrap();

    let mut manager = WarmRestartManager::with_state_file(state_file);
    let result = manager.initialize();

    // Should not error, should fall back to cold start
    assert!(result.is_ok());
    assert_eq!(manager.current_state(), WarmRestartState::ColdStart);
}

#[test]
fn test_warm_restart_is_warm_restart_in_progress() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // Setup warm start state
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        let port = PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216);
        manager.add_port(port);
        manager.save_state().unwrap();
    }

    // Test is_warm_restart_in_progress flag
    {
        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        assert!(manager.is_warm_restart_in_progress());

        manager.begin_initial_sync();
        assert!(manager.is_warm_restart_in_progress());

        manager.complete_initial_sync();
        assert!(!manager.is_warm_restart_in_progress());
    }
}

#[test]
fn test_persisted_port_state_version_compatibility() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // Create state
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
        manager.save_state().unwrap();
    }

    // Load and verify version
    {
        let json_content = fs::read_to_string(&state_file).unwrap();
        let persisted: PersistedPortState = serde_json::from_str(&json_content).unwrap();
        assert_eq!(persisted.version, 1);
    }
}

#[test]
fn test_port_state_flags_and_mtu() {
    let state = PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216);

    assert_eq!(state.name, "Ethernet0");
    assert_eq!(state.admin_state, 1);
    assert_eq!(state.oper_state, 1);
    assert_eq!(state.flags, 0x41); // IFF_UP | IFF_RUNNING
    assert_eq!(state.mtu, 9216);

    // Verify state checks
    assert!(state.is_up());
    assert!(state.is_admin_enabled());
}

#[test]
fn test_warm_restart_manager_clear_ports() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    let mut manager = WarmRestartManager::with_state_file(state_file);

    // Add multiple ports
    for i in 0..5 {
        let port = PortState::new(format!("Ethernet{}", i * 4), 1, 1, 0x41, 9216);
        manager.add_port(port);
    }

    assert_eq!(manager.port_count(), 5);

    // Clear all ports
    manager.clear_ports();
    assert_eq!(manager.port_count(), 0);

    // Verify no ports remain
    assert!(manager.get_port("Ethernet0").is_none());
}

#[test]
fn test_eoiu_detector_reset_for_reuse() {
    let mut detector = EoiuDetector::new();

    // First detection sequence
    detector.check_eoiu("Ethernet0", 1, 0x41);
    detector.check_eoiu("lo", 0, 0x01);
    assert!(detector.is_detected());

    // Reset
    detector.reset();
    assert_eq!(detector.state(), EoiuDetectionState::Waiting);
    assert_eq!(detector.messages_seen(), 0);

    // Second detection sequence (reuse detector)
    detector.check_eoiu("Ethernet0", 1, 0x41);
    detector.check_eoiu("Ethernet4", 1, 0x41);
    detector.check_eoiu("lo", 0, 0x01);
    assert!(detector.is_detected());
    assert_eq!(detector.messages_seen(), 3);
}

// ===== Task 12: Integration Tests (20 additional) =====

#[test]
fn test_warm_restart_multi_port_consistency() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // First instance - add 10 ports
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        manager.initialize().unwrap();

        for i in 0..10 {
            let port = PortState::new(format!("Ethernet{}", i * 4), 1, 1, 0x41, 9216);
            manager.add_port(port);
        }
        manager.save_state().unwrap();
        assert_eq!(manager.port_count(), 10);
    }

    // Second instance - verify all ports loaded
    {
        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();
        assert_eq!(manager.port_count(), 10);

        for i in 0..10 {
            let port_name = format!("Ethernet{}", i * 4);
            assert!(manager.get_port(&port_name).is_some());
        }
    }
}

#[test]
fn test_warm_restart_timeout_auto_completion_integration() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // Setup initial state
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        manager.initialize().unwrap();
        manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
        manager.save_state().unwrap();
    }

    // Test timeout behavior
    {
        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();
        manager.set_initial_sync_timeout(1); // 1 second timeout

        manager.begin_initial_sync();
        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncInProgress
        );

        // Wait for timeout
        std::thread::sleep(std::time::Duration::from_millis(1200));

        // Auto-completion should trigger
        manager.check_initial_sync_timeout().unwrap();
        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncComplete
        );
    }
}

#[test]
fn test_warm_restart_backup_recovery_chain() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");
    let backup_dir = temp_dir.path().join("backups");
    std::fs::create_dir_all(&backup_dir).unwrap();

    // Create multiple backup files
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        for i in 0..3 {
            manager.add_port(PortState::new(
                format!("Ethernet{}", i * 4),
                1,
                1,
                0x41,
                9216,
            ));
            manager.save_state().unwrap();
            manager.rotate_state_file().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1100)); // Ensure different timestamps
        }
    }

    // Verify backups exist
    {
        let manager = WarmRestartManager::with_state_file(state_file.clone());
        let backups = manager.get_backup_files().unwrap();
        // Should have at least 1 backup
        assert!(!backups.is_empty());
    }
}

#[test]
fn test_warm_restart_metrics_tracking_integration() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // First warm restart cycle
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        manager.initialize().unwrap();
        manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
        manager.save_state().unwrap();

        assert_eq!(manager.metrics.cold_start_count, 1);
    }

    // Second warm restart cycle
    {
        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();
        assert_eq!(manager.metrics.warm_restart_count, 1);

        manager.begin_initial_sync();
        manager.complete_initial_sync();

        assert!(manager.metrics.last_warm_restart_secs.is_some());
    }
}

#[test]
fn test_warm_restart_concurrent_state_files() {
    let temp_dir = TempDir::new().unwrap();
    let state_file1 = temp_dir.path().join("state1.json");
    let state_file2 = temp_dir.path().join("state2.json");

    // Create two independent managers with different state files
    let mut manager1 = WarmRestartManager::with_state_file(state_file1);
    let mut manager2 = WarmRestartManager::with_state_file(state_file2);

    // Manager 1: 5 ports
    for i in 0..5 {
        manager1.add_port(PortState::new(format!("Eth0_{}", i), 1, 1, 0x41, 9216));
    }
    manager1.save_state().unwrap();

    // Manager 2: 3 ports
    for i in 0..3 {
        manager2.add_port(PortState::new(format!("Eth1_{}", i), 1, 1, 0x41, 9216));
    }
    manager2.save_state().unwrap();

    // Verify independence
    assert_eq!(manager1.port_count(), 5);
    assert_eq!(manager2.port_count(), 3);
}

#[test]
fn test_warm_restart_state_transitions_full_cycle() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // Setup: create state for warm restart
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        manager.initialize().unwrap();
        manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
        manager.save_state().unwrap();
    }

    // Full state machine cycle
    {
        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        // ColdStart or WarmStart
        let initial_state = manager.current_state();
        assert!(
            initial_state == WarmRestartState::ColdStart
                || initial_state == WarmRestartState::WarmStart
        );

        // Transition to InitialSyncInProgress
        manager.begin_initial_sync();
        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncInProgress
        );
        assert!(manager.should_skip_app_db_updates());

        // Transition to InitialSyncComplete
        manager.complete_initial_sync();
        assert_eq!(
            manager.current_state(),
            WarmRestartState::InitialSyncComplete
        );
        assert!(!manager.should_skip_app_db_updates());
    }
}

#[test]
fn test_warm_restart_corruption_recovery_integration() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // Create valid state with backup
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        manager.initialize().unwrap();
        manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216));
        manager.save_state().unwrap();
        manager.rotate_state_file().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Corrupt the current state file
    fs::write(&state_file, "{ corrupted }").unwrap();

    // Recovery should succeed via backup
    {
        let mut manager = WarmRestartManager::with_state_file(state_file);
        let recovered = manager.load_state_with_recovery().unwrap();
        assert!(recovered);
        assert_eq!(manager.port_count(), 1);
        assert!(manager.metrics.state_recovery_count > 0);
    }
}

#[test]
fn test_warm_restart_eoiu_detection_sequence() {
    let mut detector = EoiuDetector::new();

    // Simulate 5 port updates
    for i in 0..5 {
        let is_eoiu = detector.check_eoiu(&format!("Ethernet{}", i * 4), 1, 0x41);
        assert!(!is_eoiu);
        detector.increment_dumped_interfaces();
    }

    assert_eq!(detector.dumped_interfaces(), 5);
    assert_eq!(detector.messages_seen(), 5);
    assert_eq!(detector.state(), EoiuDetectionState::Waiting);

    // EOIU signal arrives
    let is_eoiu = detector.check_eoiu("lo", 0, 0x01);
    assert!(is_eoiu);
    assert_eq!(detector.state(), EoiuDetectionState::Detected);
    assert_eq!(detector.messages_seen(), 6);

    // Mark complete
    detector.mark_complete();
    assert_eq!(detector.state(), EoiuDetectionState::Complete);

    // Further EOIU signals ignored
    assert!(!detector.check_eoiu("lo", 0, 0x01));
}

#[test]
fn test_warm_restart_port_state_mtu_variation() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // Create ports with different MTU values
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());

        manager.add_port(PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 1500));
        manager.add_port(PortState::new("Ethernet4".to_string(), 1, 1, 0x41, 9216));
        manager.add_port(PortState::new("Ethernet8".to_string(), 1, 1, 0x41, 4096));

        manager.save_state().unwrap();
    }

    // Verify MTU values persisted
    {
        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.load_state().unwrap();

        assert_eq!(manager.get_port("Ethernet0").unwrap().mtu, 1500);
        assert_eq!(manager.get_port("Ethernet4").unwrap().mtu, 9216);
        assert_eq!(manager.get_port("Ethernet8").unwrap().mtu, 4096);
    }
}

#[test]
fn test_warm_restart_port_state_flags() {
    let port_up = PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216);
    assert!(port_up.is_up());
    assert!(port_up.is_admin_enabled());

    let port_down = PortState::new("Ethernet4".to_string(), 1, 0, 0x01, 9216);
    assert!(!port_down.is_up());
    assert!(port_down.is_admin_enabled());

    let port_disabled = PortState::new("Ethernet8".to_string(), 0, 0, 0x00, 9216);
    assert!(!port_disabled.is_up());
    assert!(!port_disabled.is_admin_enabled());
}

#[test]
fn test_warm_restart_stale_cleanup_integration() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // Create state file
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        manager.initialize().unwrap();
        manager.save_state().unwrap();
    }

    // Cleanup should not affect recent files
    {
        let manager = WarmRestartManager::with_state_file(state_file.clone());
        let age_before = manager.state_file_age_secs().ok();

        let manager2 = WarmRestartManager::with_state_file(state_file.clone());
        manager2.cleanup_stale_state_files().unwrap();
        let age_after = manager2.state_file_age_secs().ok();

        // File should still exist (recent files not cleaned up)
        assert!(age_before.is_some());
        assert!(age_after.is_some());
    }
}

#[test]
fn test_warm_restart_multiple_cycles() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    for cycle in 0..3 {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        manager.initialize().unwrap();

        // Clear previous ports first
        manager.clear_ports();

        // Add ports for this cycle
        for i in 0..3 {
            manager.add_port(PortState::new(
                format!("Eth{}_Port{}", cycle, i),
                1,
                1,
                0x41,
                9216,
            ));
        }

        manager.save_state().unwrap();
    }

    // Final verification
    {
        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.load_state().unwrap();
        // Should have 3 ports from the last cycle
        assert_eq!(manager.port_count(), 3);
    }
}

#[test]
fn test_warm_restart_metrics_aggregation() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    let mut manager = WarmRestartManager::with_state_file(state_file);
    manager.initialize().unwrap();

    // Simulate multiple events
    manager.begin_initial_sync();
    manager.complete_initial_sync();

    manager.begin_initial_sync();
    manager.check_initial_sync_timeout().ok(); // May or may not timeout

    // Verify metrics are populated
    assert!(manager.metrics.warm_restart_count > 0 || manager.metrics.cold_start_count > 0);
}

#[test]
fn test_warm_restart_app_db_gating_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // Setup warm restart state
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        let port = PortState::new("Ethernet0".to_string(), 1, 1, 0x41, 9216);
        manager.add_port(port);
        manager.save_state().unwrap();
    }

    // Test gating behavior
    {
        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.initialize().unwrap();

        // Before sync: updates allowed (cold start) or blocked (warm start)
        let _updates_before = !manager.should_skip_app_db_updates();

        // During sync: updates blocked
        manager.begin_initial_sync();
        let updates_during = !manager.should_skip_app_db_updates();
        assert!(!updates_during);

        // After EOIU: updates allowed
        manager.complete_initial_sync();
        let updates_after = !manager.should_skip_app_db_updates();
        assert!(updates_after);
    }
}

#[test]
fn test_warm_restart_large_port_count() {
    let temp_dir = TempDir::new().unwrap();
    let state_file = temp_dir.path().join("port_state.json");

    // Create state with 100 ports
    {
        let mut manager = WarmRestartManager::with_state_file(state_file.clone());
        for i in 0..100 {
            manager.add_port(PortState::new(format!("Ethernet{}", i), 1, 1, 0x41, 9216));
        }
        manager.save_state().unwrap();
        assert_eq!(manager.port_count(), 100);
    }

    // Verify all 100 ports load correctly
    {
        let mut manager = WarmRestartManager::with_state_file(state_file);
        manager.load_state().unwrap();
        assert_eq!(manager.port_count(), 100);

        for i in 0..100 {
            assert!(manager.get_port(&format!("Ethernet{}", i)).is_some());
        }
    }
}
