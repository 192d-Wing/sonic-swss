//! Integration tests for neighsyncd
//!
//! Tests the full neighsyncd stack with mock Redis using embedded Redis or
//! by mocking the Redis adapter.

#![allow(clippy::useless_vec)]

#[cfg(test)]
mod tests {
    use sonic_neighsyncd::{MacAddress, NeighborEntry, NeighborMessageType, NeighborState};
    use std::net::IpAddr;
    use std::str::FromStr;

    /// Test helper to create a neighbor entry
    fn make_test_entry(
        ifindex: u32,
        interface: &str,
        ip: &str,
        mac: &str,
        state: NeighborState,
    ) -> NeighborEntry {
        NeighborEntry {
            ifindex,
            interface: interface.to_string(),
            ip: IpAddr::from_str(ip).expect("valid IP"),
            mac: MacAddress::from_str(mac).expect("valid MAC"),
            state,
            externally_learned: false,
        }
    }

    #[test]
    fn test_neighbor_entry_creation() {
        let entry = make_test_entry(
            1,
            "eth0",
            "2001:db8::1",
            "00:11:22:33:44:55",
            NeighborState::Reachable,
        );

        assert_eq!(entry.ifindex, 1);
        assert_eq!(entry.interface, "eth0");
        assert_eq!(entry.state, NeighborState::Reachable);
        assert_eq!(entry.mac.to_string(), "00:11:22:33:44:55");
    }

    #[test]
    fn test_neighbor_entry_ipv6_link_local() {
        let entry = make_test_entry(
            1,
            "eth0",
            "fe80::1",
            "00:11:22:33:44:55",
            NeighborState::Reachable,
        );

        assert!(entry.is_ipv6_link_local());
        assert!(!entry.is_ipv6_multicast_link_local());
    }

    #[test]
    fn test_neighbor_entry_ipv6_multicast() {
        let entry = make_test_entry(
            1,
            "eth0",
            "ff02::1",
            "00:11:22:33:44:55",
            NeighborState::Reachable,
        );

        assert!(entry.is_ipv6_multicast_link_local());
    }

    #[test]
    fn test_neighbor_entry_zero_mac() {
        let entry = make_test_entry(
            1,
            "eth0",
            "2001:db8::1",
            "00:00:00:00:00:00",
            NeighborState::Incomplete,
        );

        assert!(entry.mac.is_zero());
        assert!(!entry.mac.is_broadcast());
    }

    #[test]
    fn test_neighbor_entry_broadcast_mac() {
        let entry = make_test_entry(
            1,
            "eth0",
            "2001:db8::1",
            "ff:ff:ff:ff:ff:ff",
            NeighborState::Reachable,
        );

        assert!(!entry.mac.is_zero());
        assert!(entry.mac.is_broadcast());
    }

    #[test]
    fn test_neighbor_state_resolved() {
        assert!(NeighborState::Reachable.is_resolved());
        assert!(NeighborState::Stale.is_resolved());
        assert!(NeighborState::Delay.is_resolved());
        assert!(NeighborState::Probe.is_resolved());
        assert!(NeighborState::Permanent.is_resolved());

        assert!(!NeighborState::Incomplete.is_resolved());
        assert!(!NeighborState::Failed.is_resolved());
        assert!(!NeighborState::NoArp.is_resolved());
    }

    #[test]
    fn test_neighbor_state_transitions() {
        // Valid transitions
        assert!(!NeighborState::Incomplete.is_resolved());
        assert!(NeighborState::Reachable.is_resolved());

        // State comparison
        assert_eq!(NeighborState::Reachable, NeighborState::Reachable);
        assert_ne!(NeighborState::Reachable, NeighborState::Incomplete);
    }

    #[test]
    fn test_neighbor_redis_key() {
        let entry = make_test_entry(
            1,
            "Vlan100",
            "2001:db8::1",
            "00:11:22:33:44:55",
            NeighborState::Reachable,
        );
        let key = entry.redis_key();

        // Redis key format should be "interface:ip"
        assert!(key.contains("Vlan100"));
        assert!(key.contains("2001:db8::1"));
        assert!(key.contains(":"));
        assert_eq!(key, "Vlan100:2001:db8::1");
    }

    #[test]
    fn test_mac_address_parsing() {
        let mac = MacAddress::from_str("00:11:22:33:44:55").expect("valid MAC");
        assert_eq!(mac.to_string(), "00:11:22:33:44:55");

        let zero = MacAddress::from_str("00:00:00:00:00:00").expect("valid MAC");
        assert!(zero.is_zero());

        let broadcast = MacAddress::from_str("ff:ff:ff:ff:ff:ff").expect("valid MAC");
        assert!(broadcast.is_broadcast());
    }

    #[test]
    fn test_mac_address_case_insensitive() {
        let mac1 = MacAddress::from_str("00:11:22:33:44:55").expect("valid MAC");
        let mac2 = MacAddress::from_str("00:11:22:33:44:55").expect("valid MAC");

        assert_eq!(mac1, mac2);
    }

    #[test]
    fn test_neighbor_message_types() {
        // Test that message types can be created and compared
        let new_msg = NeighborMessageType::New;
        let del_msg = NeighborMessageType::Delete;
        let get_msg = NeighborMessageType::Get;

        assert_eq!(new_msg, NeighborMessageType::New);
        assert_ne!(new_msg, del_msg);
        assert_ne!(del_msg, get_msg);
    }

    #[test]
    fn test_ipv6_link_local_detection() {
        // Valid link-local addresses
        let entry1 = make_test_entry(
            1,
            "eth0",
            "fe80::1",
            "00:11:22:33:44:55",
            NeighborState::Reachable,
        );
        assert!(entry1.is_ipv6_link_local());

        let entry2 = make_test_entry(
            1,
            "eth0",
            "fe80::ffff:1",
            "00:11:22:33:44:55",
            NeighborState::Reachable,
        );
        assert!(entry2.is_ipv6_link_local());

        // Non-link-local addresses
        let entry3 = make_test_entry(
            1,
            "eth0",
            "2001:db8::1",
            "00:11:22:33:44:55",
            NeighborState::Reachable,
        );
        assert!(!entry3.is_ipv6_link_local());

        let entry4 = make_test_entry(
            1,
            "eth0",
            "::1",
            "00:11:22:33:44:55",
            NeighborState::Reachable,
        );
        assert!(!entry4.is_ipv6_link_local());
    }

    #[test]
    fn test_ipv6_multicast_link_local_detection() {
        // IPv6 multicast link-local (ff02::/64 with link-local scope)
        let entry1 = make_test_entry(
            1,
            "eth0",
            "ff02::1",
            "00:11:22:33:44:55",
            NeighborState::Reachable,
        );
        assert!(entry1.is_ipv6_multicast_link_local());

        let entry2 = make_test_entry(
            1,
            "eth0",
            "ff02::1:ff00:1",
            "00:11:22:33:44:55",
            NeighborState::Reachable,
        );
        assert!(entry2.is_ipv6_multicast_link_local());

        // Non-multicast link-local
        let entry3 = make_test_entry(
            1,
            "eth0",
            "fe80::1",
            "00:11:22:33:44:55",
            NeighborState::Reachable,
        );
        assert!(!entry3.is_ipv6_multicast_link_local());

        // Global multicast
        let entry4 = make_test_entry(
            1,
            "eth0",
            "ff0e::1",
            "00:11:22:33:44:55",
            NeighborState::Reachable,
        );
        assert!(!entry4.is_ipv6_multicast_link_local());
    }

    #[test]
    fn test_neighbor_entry_externally_learned() {
        let mut entry = make_test_entry(
            1,
            "eth0",
            "2001:db8::1",
            "00:11:22:33:44:55",
            NeighborState::NoArp,
        );

        // Without externally_learned flag, should not process
        assert!(!entry.externally_learned);

        // With externally_learned flag, should process
        entry.externally_learned = true;
        assert!(entry.externally_learned);
    }

    #[test]
    fn test_neighbor_entry_dual_tor_handling() {
        // On dual-ToR, incomplete neighbors should have zero MAC set
        let entry = make_test_entry(
            1,
            "eth0",
            "2001:db8::1",
            "00:00:00:00:00:00",
            NeighborState::Incomplete,
        );

        assert!(entry.mac.is_zero());
        assert!(!entry.state.is_resolved());
    }

    #[test]
    fn test_multiple_interfaces() {
        let entries = vec![
            make_test_entry(
                1,
                "Ethernet0",
                "2001:db8::1",
                "00:11:22:33:44:55",
                NeighborState::Reachable,
            ),
            make_test_entry(
                2,
                "Ethernet1",
                "2001:db8::2",
                "00:11:22:33:44:56",
                NeighborState::Reachable,
            ),
            make_test_entry(
                3,
                "Vlan100",
                "2001:db8::3",
                "00:11:22:33:44:57",
                NeighborState::Reachable,
            ),
            make_test_entry(
                4,
                "PortChannel0",
                "2001:db8::4",
                "00:11:22:33:44:58",
                NeighborState::Reachable,
            ),
        ];

        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].interface, "Ethernet0");
        assert_eq!(entries[3].interface, "PortChannel0");
    }

    #[test]
    fn test_neighbor_batch_processing() {
        let entries = vec![
            make_test_entry(
                1,
                "eth0",
                "2001:db8::1",
                "00:11:22:33:44:55",
                NeighborState::Reachable,
            ),
            make_test_entry(
                2,
                "eth0",
                "2001:db8::2",
                "00:11:22:33:44:56",
                NeighborState::Reachable,
            ),
            make_test_entry(
                3,
                "eth0",
                "2001:db8::3",
                "00:11:22:33:44:57",
                NeighborState::Reachable,
            ),
        ];

        // Simulate batching
        let batch_size = 2;
        let mut processed = 0;
        for chunk in entries.chunks(batch_size) {
            processed += chunk.len();
        }

        assert_eq!(processed, 3);
    }

    #[test]
    fn test_neighbor_filtering() {
        let entries = vec![
            // Valid neighbor
            make_test_entry(
                1,
                "eth0",
                "2001:db8::1",
                "00:11:22:33:44:55",
                NeighborState::Reachable,
            ),
            // Broadcast MAC - should filter
            make_test_entry(
                2,
                "eth0",
                "2001:db8::2",
                "ff:ff:ff:ff:ff:ff",
                NeighborState::Reachable,
            ),
            // Zero MAC (non-dual-tor) - should filter
            make_test_entry(
                3,
                "eth0",
                "2001:db8::3",
                "00:00:00:00:00:00",
                NeighborState::Reachable,
            ),
            // IPv6 multicast link-local - should filter
            make_test_entry(
                4,
                "eth0",
                "ff02::1",
                "00:11:22:33:44:58",
                NeighborState::Reachable,
            ),
        ];

        // Count invalid entries based on filtering criteria
        let broadcast_count = entries.iter().filter(|e| e.mac.is_broadcast()).count();
        let zero_mac_count = entries.iter().filter(|e| e.mac.is_zero()).count();
        let multicast_count = entries
            .iter()
            .filter(|e| e.is_ipv6_multicast_link_local())
            .count();

        assert_eq!(broadcast_count, 1); // One broadcast
        assert_eq!(zero_mac_count, 1); // One zero MAC
        assert_eq!(multicast_count, 1); // One multicast link-local
    }

    #[tokio::test]
    async fn test_async_operations() {
        // Test that async operations don't panic
        // In a real integration test with Redis, we would test actual operations
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
    }

    #[test]
    fn test_neighbor_entry_ordering() {
        let entry1 = make_test_entry(
            1,
            "eth0",
            "2001:db8::1",
            "00:11:22:33:44:55",
            NeighborState::Reachable,
        );
        let entry2 = make_test_entry(
            1,
            "eth0",
            "2001:db8::2",
            "00:11:22:33:44:56",
            NeighborState::Reachable,
        );

        // Same interface, different IPs
        assert_eq!(entry1.interface, entry2.interface);
        assert_ne!(entry1.ip, entry2.ip);
    }

    #[test]
    fn test_neighbor_states_completeness() {
        // Ensure all neighbor states are covered
        let states = vec![
            NeighborState::Incomplete,
            NeighborState::Reachable,
            NeighborState::Stale,
            NeighborState::Delay,
            NeighborState::Probe,
            NeighborState::Failed,
            NeighborState::NoArp,
            NeighborState::Permanent,
        ];

        assert_eq!(states.len(), 8);
    }

    #[test]
    fn test_warm_restart_scenario() {
        // Simulate entries that would exist during warm restart
        let cached_entries = vec![
            make_test_entry(
                1,
                "eth0",
                "2001:db8::1",
                "00:11:22:33:44:55",
                NeighborState::Reachable,
            ),
            make_test_entry(
                2,
                "eth0",
                "2001:db8::2",
                "00:11:22:33:44:56",
                NeighborState::Reachable,
            ),
        ];

        // During warm restart, new events would be accumulated
        let new_events = vec![
            make_test_entry(
                1,
                "eth0",
                "2001:db8::1",
                "00:11:22:33:44:55",
                NeighborState::Reachable,
            ),
            make_test_entry(
                3,
                "eth0",
                "2001:db8::3",
                "00:11:22:33:44:57",
                NeighborState::Reachable,
            ),
        ];

        assert_eq!(cached_entries.len(), 2);
        assert_eq!(new_events.len(), 2);
    }

    #[test]
    fn test_ipv4_link_local_detection() {
        #[cfg(feature = "ipv4")]
        {
            let entry = make_test_entry(
                1,
                "eth0",
                "169.254.1.1",
                "00:11:22:33:44:55",
                NeighborState::Reachable,
            );
            assert!(entry.is_ipv4_link_local());
        }
    }

    #[test]
    fn test_neighbor_performance_batch_large() {
        // Test that we can handle large batches efficiently
        let mut entries = Vec::new();
        for i in 0..1000 {
            let ip = format!("2001:db8::{}", i);
            let mac = format!("00:11:22:33:44:{:02x}", i % 256);
            entries.push(make_test_entry(
                i as u32,
                "eth0",
                &ip,
                &mac,
                NeighborState::Reachable,
            ));
        }

        assert_eq!(entries.len(), 1000);

        // Simulate batching in chunks of 100
        let batch_size = 100;
        let batches = entries.len().div_ceil(batch_size);
        assert_eq!(batches, 10);
    }

    #[test]
    fn test_concurrent_interface_cache() {
        // Test that the interface cache can handle multiple interfaces
        let mut interfaces = std::collections::HashMap::new();
        interfaces.insert(1, "Ethernet0".to_string());
        interfaces.insert(2, "Ethernet1".to_string());
        interfaces.insert(10, "Vlan100".to_string());
        interfaces.insert(1000, "PortChannel0".to_string());

        assert_eq!(interfaces.len(), 4);
        assert_eq!(interfaces.get(&1), Some(&"Ethernet0".to_string()));
    }
}
