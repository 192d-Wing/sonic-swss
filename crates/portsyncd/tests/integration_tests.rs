//! Integration tests for portsyncd daemon
//!
//! Tests full port synchronization workflow including:
//! - Port configuration loading
//! - Netlink event handling
//! - Database state updates
//! - Initialization signaling

use sonic_portsyncd::{
    DatabaseConnection, LinkStatus, LinkSync, NetlinkEvent, NetlinkEventType, PortConfig,
    PortLinkState, load_port_config, send_port_config_done, send_port_init_done,
};

/// Test fixture: Setup mock databases with port configuration
struct TestSetup {
    config_db: DatabaseConnection,
    app_db: DatabaseConnection,
    state_db: DatabaseConnection,
}

impl TestSetup {
    fn new() -> Self {
        Self {
            config_db: DatabaseConnection::new("CONFIG_DB".to_string()),
            app_db: DatabaseConnection::new("APP_DB".to_string()),
            state_db: DatabaseConnection::new("STATE_DB".to_string()),
        }
    }

    /// Add a port configuration to CONFIG_DB
    async fn add_port_config(
        &mut self,
        port_name: &str,
        speed: &str,
        lanes: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let key = format!("PORT|{}", port_name);
        let fields = vec![
            ("speed".to_string(), speed.to_string()),
            ("lanes".to_string(), lanes.to_string()),
        ];
        self.config_db.hset(&key, &fields).await?;
        Ok(())
    }
}

#[tokio::test]
async fn test_full_port_sync_workflow() {
    let mut setup = TestSetup::new();

    // Add port configurations
    setup
        .add_port_config("Ethernet0", "100G", "0,1,2,3")
        .await
        .expect("Failed to add port config");
    setup
        .add_port_config("Ethernet4", "100G", "4,5,6,7")
        .await
        .expect("Failed to add port config");

    // Load port configuration
    let ports = load_port_config(&setup.config_db, &mut setup.app_db, false)
        .await
        .expect("Failed to load port config");

    assert_eq!(ports.len(), 2);
    let port_names: std::collections::HashSet<_> = ports.iter().map(|p| p.name.as_str()).collect();
    assert!(port_names.contains("Ethernet0"));
    assert!(port_names.contains("Ethernet4"));

    // Send PortConfigDone signal
    send_port_config_done(&mut setup.app_db)
        .await
        .expect("Failed to send config done");

    // Verify signal was sent
    let result = setup
        .app_db
        .hgetall("PortConfigDone")
        .await
        .expect("Failed to read signal");
    assert!(!result.is_empty());
}

#[tokio::test]
async fn test_port_initialization_flow() {
    let mut setup = TestSetup::new();
    let mut link_sync = LinkSync::new().expect("Failed to create LinkSync");

    // Initialize with two ports
    link_sync.initialize_ports(vec!["Ethernet0".to_string(), "Ethernet4".to_string()]);

    assert_eq!(link_sync.uninitialized_count(), 2);
    assert!(!link_sync.should_send_port_init_done());

    // Simulate RTM_NEWLINK events for both ports
    let event1 = NetlinkEvent {
        event_type: NetlinkEventType::NewLink,
        port_name: "Ethernet0".to_string(),
        flags: Some(0x1), // Up
        mtu: Some(9100),
    };

    link_sync
        .handle_new_link(&event1, &mut setup.state_db)
        .await
        .expect("Failed to handle event");

    assert_eq!(link_sync.uninitialized_count(), 1);
    assert!(!link_sync.should_send_port_init_done());

    let event2 = NetlinkEvent {
        event_type: NetlinkEventType::NewLink,
        port_name: "Ethernet4".to_string(),
        flags: Some(0x1),
        mtu: Some(9100),
    };

    link_sync
        .handle_new_link(&event2, &mut setup.state_db)
        .await
        .expect("Failed to handle event");

    assert_eq!(link_sync.uninitialized_count(), 0);
    assert!(link_sync.should_send_port_init_done());

    // Send PortInitDone signal
    send_port_init_done(&mut setup.app_db)
        .await
        .expect("Failed to send init done");

    link_sync.set_port_init_done();
    assert!(!link_sync.should_send_port_init_done());
}

#[tokio::test]
async fn test_port_state_updates_in_state_db() {
    let mut setup = TestSetup::new();
    let mut link_sync = LinkSync::new().expect("Failed to create LinkSync");

    // Initialize ports
    link_sync.initialize_ports(vec!["Ethernet0".to_string()]);

    // Receive RTM_NEWLINK with link up
    let event_up = NetlinkEvent {
        event_type: NetlinkEventType::NewLink,
        port_name: "Ethernet0".to_string(),
        flags: Some(0x1),
        mtu: Some(9100),
    };

    link_sync
        .handle_new_link(&event_up, &mut setup.state_db)
        .await
        .expect("Failed to handle up event");

    // Verify port status in STATE_DB
    let state = setup
        .state_db
        .hgetall("PORT_TABLE|Ethernet0")
        .await
        .expect("Failed to read state");

    assert_eq!(state.get("mtu"), Some(&"9100".to_string()));
    assert_eq!(state.get("netdev_oper_status"), Some(&"up".to_string()));

    // Simulate link going down
    let event_down = NetlinkEvent {
        event_type: NetlinkEventType::NewLink,
        port_name: "Ethernet0".to_string(),
        flags: Some(0x0), // Down
        mtu: Some(9100),
    };

    link_sync
        .handle_new_link(&event_down, &mut setup.state_db)
        .await
        .expect("Failed to handle down event");

    // Verify port status changed
    let state = setup
        .state_db
        .hgetall("PORT_TABLE|Ethernet0")
        .await
        .expect("Failed to read state");

    assert_eq!(state.get("netdev_oper_status"), Some(&"down".to_string()));
}

#[tokio::test]
async fn test_port_deletion_from_state_db() {
    let mut setup = TestSetup::new();
    let mut link_sync = LinkSync::new().expect("Failed to create LinkSync");

    // Add port to state
    let event = NetlinkEvent {
        event_type: NetlinkEventType::NewLink,
        port_name: "Ethernet0".to_string(),
        flags: Some(0x1),
        mtu: Some(9100),
    };

    link_sync
        .handle_new_link(&event, &mut setup.state_db)
        .await
        .expect("Failed to add port");

    // Verify port exists
    let state = setup
        .state_db
        .hgetall("PORT_TABLE|Ethernet0")
        .await
        .expect("Failed to read state");
    assert!(!state.is_empty());

    // Delete port via RTM_DELLINK
    link_sync
        .handle_del_link("Ethernet0", &mut setup.state_db)
        .await
        .expect("Failed to delete port");

    // Verify port is removed
    let state = setup
        .state_db
        .hgetall("PORT_TABLE|Ethernet0")
        .await
        .expect("Failed to read state");
    assert!(state.is_empty());
}

#[tokio::test]
async fn test_multi_port_convergence() {
    let mut setup = TestSetup::new();
    let mut link_sync = LinkSync::new().expect("Failed to create LinkSync");

    // Configure 8 ports
    let port_count = 8;
    let mut port_names = Vec::new();
    for i in 0..port_count {
        port_names.push(format!("Ethernet{}", i * 4));
    }

    link_sync.initialize_ports(port_names.clone());
    assert_eq!(link_sync.uninitialized_count(), port_count);

    // Simulate RTM_NEWLINK events for all ports
    for (idx, port_name) in port_names.iter().enumerate() {
        let event = NetlinkEvent {
            event_type: NetlinkEventType::NewLink,
            port_name: port_name.clone(),
            flags: Some(if idx % 2 == 0 { 0x1 } else { 0x0 }), // Alternating up/down
            mtu: Some(9100),
        };

        link_sync
            .handle_new_link(&event, &mut setup.state_db)
            .await
            .expect("Failed to handle event");
    }

    assert_eq!(link_sync.uninitialized_count(), 0);
    assert!(link_sync.are_all_ports_initialized());
    assert!(link_sync.should_send_port_init_done());

    // Verify all ports in STATE_DB
    for port_name in port_names {
        let state = setup
            .state_db
            .hgetall(&format!("PORT_TABLE|{}", port_name))
            .await
            .expect("Failed to read state");
        assert!(
            !state.is_empty(),
            "Port {} not found in STATE_DB",
            port_name
        );
    }
}

#[tokio::test]
async fn test_port_config_validation() {
    // Test port with valid configuration
    let mut valid_config = PortConfig::new("Ethernet0".to_string());
    valid_config.lanes = Some("0,1,2,3".to_string());
    valid_config.mtu = Some("9100".to_string());

    assert!(
        valid_config.validate().is_ok(),
        "Valid config should pass validation"
    );

    // Test port with invalid MTU
    let mut invalid_mtu = PortConfig::new("Ethernet0".to_string());
    invalid_mtu.mtu = Some("invalid_value".to_string());

    assert!(
        invalid_mtu.validate().is_err(),
        "Invalid MTU should fail validation"
    );

    // Test port with empty lanes
    let mut empty_lanes = PortConfig::new("Ethernet0".to_string());
    empty_lanes.lanes = Some("".to_string());

    assert!(
        empty_lanes.validate().is_err(),
        "Empty lanes should fail validation"
    );
}

#[tokio::test]
async fn test_interface_filtering() {
    let mut setup = TestSetup::new();
    let mut link_sync = LinkSync::new().expect("Failed to create LinkSync");

    // Initialize with front-panel port
    link_sync.initialize_ports(vec!["Ethernet0".to_string()]);

    // Try to add management interface (should be ignored)
    let eth0_event = NetlinkEvent {
        event_type: NetlinkEventType::NewLink,
        port_name: "eth0".to_string(),
        flags: Some(0x1),
        mtu: Some(1500),
    };

    link_sync
        .handle_new_link(&eth0_event, &mut setup.state_db)
        .await
        .expect("Should ignore eth0");

    // eth0 should not be in STATE_DB
    let state = setup
        .state_db
        .hgetall("PORT_TABLE|eth0")
        .await
        .expect("Failed to read state");
    assert!(state.is_empty(), "eth0 should not be in STATE_DB");

    // Add front-panel port (should work)
    let ethernet0_event = NetlinkEvent {
        event_type: NetlinkEventType::NewLink,
        port_name: "Ethernet0".to_string(),
        flags: Some(0x1),
        mtu: Some(9100),
    };

    link_sync
        .handle_new_link(&ethernet0_event, &mut setup.state_db)
        .await
        .expect("Failed to handle Ethernet0");

    // Ethernet0 should be in STATE_DB
    let state = setup
        .state_db
        .hgetall("PORT_TABLE|Ethernet0")
        .await
        .expect("Failed to read state");
    assert!(!state.is_empty(), "Ethernet0 should be in STATE_DB");
}

#[tokio::test]
async fn test_port_channel_support() {
    let mut setup = TestSetup::new();
    let mut link_sync = LinkSync::new().expect("Failed to create LinkSync");

    // Initialize with PortChannel
    link_sync.initialize_ports(vec!["PortChannel001".to_string()]);

    let event = NetlinkEvent {
        event_type: NetlinkEventType::NewLink,
        port_name: "PortChannel001".to_string(),
        flags: Some(0x1),
        mtu: Some(9100),
    };

    link_sync
        .handle_new_link(&event, &mut setup.state_db)
        .await
        .expect("Failed to handle PortChannel");

    // Verify PortChannel in STATE_DB
    let state = setup
        .state_db
        .hgetall("PORT_TABLE|PortChannel001")
        .await
        .expect("Failed to read state");
    assert!(!state.is_empty(), "PortChannel should be in STATE_DB");
}

#[tokio::test]
async fn test_warm_restart_skips_app_db() {
    let mut setup = TestSetup::new();

    // Add port configuration
    setup
        .add_port_config("Ethernet0", "100G", "0,1,2,3")
        .await
        .expect("Failed to add port config");

    // Load with warm_restart=true (should skip APP_DB writes)
    let ports = load_port_config(&setup.config_db, &mut setup.app_db, true)
        .await
        .expect("Failed to load port config");

    assert_eq!(ports.len(), 1);

    // Verify APP_DB was not written to (no PORT_TABLE entry)
    let app_state = setup
        .app_db
        .hgetall("PORT_TABLE|Ethernet0")
        .await
        .expect("Failed to read APP_DB");
    assert!(
        app_state.is_empty(),
        "APP_DB should not be written during warm restart"
    );
}

#[tokio::test]
async fn test_port_mtu_extraction_from_netlink() {
    let state = PortLinkState::new(
        "Ethernet0".to_string(),
        LinkStatus::Up,
        LinkStatus::Up,
        9100,
    );

    let fields = state.to_field_values();
    let mtu_field = fields
        .iter()
        .find(|(k, _)| k == "mtu")
        .expect("MTU field not found");

    assert_eq!(mtu_field.1, "9100");
}

#[tokio::test]
async fn test_admin_status_from_config() {
    // Admin status should come from CONFIG_DB in production
    // This test verifies the field serialization
    let config = PortConfig::new("Ethernet0".to_string());
    let fields = config.to_field_values();

    // admin_status should not be in fields if not set
    let has_admin_status = fields.iter().any(|(k, _)| k == "admin_status");
    assert!(!has_admin_status, "Unset admin_status should not appear");
}

#[tokio::test]
async fn test_netlink_flag_combinations() {
    // Test various flag combinations
    let test_cases = vec![
        (0x1, LinkStatus::Up),    // IFF_UP only
        (0x0, LinkStatus::Down),  // No IFF_UP
        (0x41, LinkStatus::Up),   // IFF_UP | other flags
        (0x40, LinkStatus::Down), // Other flags without IFF_UP
    ];

    for (flags, expected_status) in test_cases {
        let status = LinkStatus::from_netlink_flags(flags);
        assert_eq!(status, expected_status, "Failed for flags: {}", flags);
    }
}
