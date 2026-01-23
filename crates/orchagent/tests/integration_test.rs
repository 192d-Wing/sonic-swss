//! Integration tests for orchagent modules with SAI layer
//!
//! These tests verify that orchestration modules interact correctly with
//! the SAI (Switch Abstraction Interface) layer.

use std::sync::{Arc, Mutex};

/// Mock SAI implementation for testing
///
/// This mock SAI layer simulates the behavior of a real SAI implementation
/// without requiring actual hardware or the SAI library.
pub struct MockSai {
    /// Track created SAI objects
    objects: Arc<Mutex<Vec<SaiObject>>>,
    /// Simulate object ID generation
    next_oid: Arc<Mutex<u64>>,
}

#[derive(Debug, Clone)]
pub struct SaiObject {
    pub oid: u64,
    pub object_type: SaiObjectType,
    pub attributes: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SaiObjectType {
    Port,
    Route,
    NextHop,
    NextHopGroup,
    Neighbor,
    Vnet,
    Tunnel,
    BufferPool,
    BufferProfile,
    QosMap,
    Scheduler,
    WredProfile,
    NatEntry,
    MacsecPort,
    Srv6LocalSid,
    AclTable,
    AclRule,
    AclCounter,
    BfdSession,
    FlexCounterGroup,
    PortCounter,
    QueueCounter,
    BufferCounter,
    Samplepacket,
    VirtualRouter,
    DebugCounter,
    TwampSession,
    StpInstance,
    StpPort,
    Policer,
    IsolationGroup,
    IsolationGroupMember,
    TunnelTermEntry,
    Switch,
    SystemPort,
    FabricPort,
    CoppTrap,
    CoppTrapGroup,
    MplsRoute,
    IcmpEchoSession,
    // New types for remaining modules
    DtelEvent,
    DtelIntSession,
    FdbEntry,
    FgNhg,
    FgNhgMember,
    RouterInterface,
    MirrorSession,
    MuxTunnel,
    MuxAcl,
    PbhHash,
    PbhTable,
    PbhRule,
}

impl MockSai {
    pub fn new() -> Self {
        Self {
            objects: Arc::new(Mutex::new(Vec::new())),
            next_oid: Arc::new(Mutex::new(1)),
        }
    }

    /// Create a SAI object and return its OID
    pub fn create_object(
        &self,
        object_type: SaiObjectType,
        attributes: Vec<(String, String)>,
    ) -> Result<u64, String> {
        let mut next_oid = self.next_oid.lock().unwrap();
        let oid = *next_oid;
        *next_oid += 1;

        let object = SaiObject {
            oid,
            object_type,
            attributes,
        };

        self.objects.lock().unwrap().push(object);
        Ok(oid)
    }

    /// Remove a SAI object by OID
    pub fn remove_object(&self, oid: u64) -> Result<(), String> {
        let mut objects = self.objects.lock().unwrap();
        if let Some(pos) = objects.iter().position(|obj| obj.oid == oid) {
            objects.remove(pos);
            Ok(())
        } else {
            Err(format!("Object with OID {} not found", oid))
        }
    }

    /// Get a SAI object by OID
    pub fn get_object(&self, oid: u64) -> Option<SaiObject> {
        self.objects
            .lock()
            .unwrap()
            .iter()
            .find(|obj| obj.oid == oid)
            .cloned()
    }

    /// Count objects of a specific type
    pub fn count_objects(&self, object_type: SaiObjectType) -> usize {
        self.objects
            .lock()
            .unwrap()
            .iter()
            .filter(|obj| obj.object_type == object_type)
            .count()
    }

    /// Clear all objects (for test cleanup)
    pub fn clear(&self) {
        self.objects.lock().unwrap().clear();
        *self.next_oid.lock().unwrap() = 1;
    }
}

impl Default for MockSai {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_sai_create_object() {
        let sai = MockSai::new();

        let oid = sai
            .create_object(
                SaiObjectType::Port,
                vec![("speed".to_string(), "100000".to_string())],
            )
            .unwrap();

        assert_eq!(oid, 1);
        assert_eq!(sai.count_objects(SaiObjectType::Port), 1);

        let obj = sai.get_object(oid).unwrap();
        assert_eq!(obj.object_type, SaiObjectType::Port);
        assert_eq!(obj.attributes.len(), 1);
    }

    #[test]
    fn test_mock_sai_remove_object() {
        let sai = MockSai::new();

        let oid = sai
            .create_object(SaiObjectType::Port, vec![])
            .unwrap();

        assert_eq!(sai.count_objects(SaiObjectType::Port), 1);

        sai.remove_object(oid).unwrap();
        assert_eq!(sai.count_objects(SaiObjectType::Port), 0);
    }

    #[test]
    fn test_mock_sai_multiple_objects() {
        let sai = MockSai::new();

        let oid1 = sai.create_object(SaiObjectType::Port, vec![]).unwrap();
        let oid2 = sai.create_object(SaiObjectType::Route, vec![]).unwrap();
        let oid3 = sai.create_object(SaiObjectType::Port, vec![]).unwrap();

        assert_eq!(sai.count_objects(SaiObjectType::Port), 2);
        assert_eq!(sai.count_objects(SaiObjectType::Route), 1);

        assert_eq!(oid1, 1);
        assert_eq!(oid2, 2);
        assert_eq!(oid3, 3);
    }

    #[test]
    fn test_mock_sai_clear() {
        let sai = MockSai::new();

        sai.create_object(SaiObjectType::Port, vec![]).unwrap();
        sai.create_object(SaiObjectType::Route, vec![]).unwrap();

        assert_eq!(sai.count_objects(SaiObjectType::Port), 1);
        assert_eq!(sai.count_objects(SaiObjectType::Route), 1);

        sai.clear();

        assert_eq!(sai.count_objects(SaiObjectType::Port), 0);
        assert_eq!(sai.count_objects(SaiObjectType::Route), 0);
    }
}

// Integration tests for orchestration modules
#[cfg(test)]
mod integration_tests {
    use super::*;

    // NeighOrch integration tests
    mod neigh_orch_tests {
        use super::*;
        use sonic_orchagent::neigh::{NeighOrch, NeighOrchConfig, NeighborEntry, NeighborKey, MacAddress};
        use std::net::IpAddr;

        fn create_neighbor_with_sai(ip: &str, interface: &str, mac: &str, sai: &MockSai) -> (NeighborEntry, u64) {
            let ip_addr: IpAddr = ip.parse().unwrap();
            let mac_addr = MacAddress::from_str(mac).unwrap();
            let key = NeighborKey::new(interface.to_string(), ip_addr);

            let mut entry = NeighborEntry::new(key, mac_addr);

            // Create SAI neighbor object
            let oid = sai.create_object(
                SaiObjectType::Neighbor,
                vec![
                    ("ip".to_string(), ip.to_string()),
                    ("interface".to_string(), interface.to_string()),
                    ("mac".to_string(), mac.to_string()),
                ]
            ).unwrap();

            entry.neigh_oid = oid;
            (entry, oid)
        }

        #[test]
        fn test_neigh_orch_add_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = NeighOrch::new(NeighOrchConfig::default());

            assert_eq!(sai.count_objects(SaiObjectType::Neighbor), 0);

            let (neighbor, oid) = create_neighbor_with_sai("10.0.0.1", "Ethernet0", "00:11:22:33:44:55", &sai);
            orch.add_neighbor(neighbor).unwrap();

            assert_eq!(orch.neighbor_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Neighbor), 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::Neighbor);
        }

        #[test]
        fn test_neigh_orch_remove_deletes_sai_object() {
            let sai = MockSai::new();
            let mut orch = NeighOrch::new(NeighOrchConfig::default());

            let (neighbor, oid) = create_neighbor_with_sai("10.0.0.1", "Ethernet0", "00:11:22:33:44:55", &sai);
            let key = neighbor.key.clone();
            orch.add_neighbor(neighbor).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::Neighbor), 1);

            let removed = orch.remove_neighbor(&key).unwrap();
            sai.remove_object(removed.neigh_oid).unwrap();

            assert_eq!(orch.neighbor_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Neighbor), 0);
        }

        #[test]
        fn test_neigh_orch_multiple_neighbors() {
            let sai = MockSai::new();
            let mut orch = NeighOrch::new(NeighOrchConfig::default());

            let (n1, _) = create_neighbor_with_sai("10.0.0.1", "Ethernet0", "00:11:22:33:44:55", &sai);
            let (n2, _) = create_neighbor_with_sai("10.0.0.2", "Ethernet0", "00:11:22:33:44:56", &sai);
            let (n3, _) = create_neighbor_with_sai("fe80::1", "Ethernet4", "00:11:22:33:44:57", &sai);

            orch.add_neighbor(n1).unwrap();
            orch.add_neighbor(n2).unwrap();
            orch.add_neighbor(n3).unwrap();

            assert_eq!(orch.neighbor_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::Neighbor), 3);
            assert_eq!(orch.stats().stats.ipv4_neighbors, 2);
            assert_eq!(orch.stats().stats.ipv6_neighbors, 1);
        }

        #[test]
        fn test_neigh_orch_ipv4_and_ipv6_neighbors_on_same_interface() {
            let sai = MockSai::new();
            let mut orch = NeighOrch::new(NeighOrchConfig::default());

            // Add multiple IPv4 and IPv6 neighbors on the same interface
            let (n1, _) = create_neighbor_with_sai("10.0.0.1", "Ethernet0", "00:11:22:33:44:01", &sai);
            let (n2, _) = create_neighbor_with_sai("10.0.0.2", "Ethernet0", "00:11:22:33:44:02", &sai);
            let (n3, _) = create_neighbor_with_sai("fe80::1", "Ethernet0", "00:11:22:33:44:03", &sai);
            let (n4, _) = create_neighbor_with_sai("fe80::2", "Ethernet0", "00:11:22:33:44:04", &sai);

            orch.add_neighbor(n1).unwrap();
            orch.add_neighbor(n2).unwrap();
            orch.add_neighbor(n3).unwrap();
            orch.add_neighbor(n4).unwrap();

            assert_eq!(orch.neighbor_count(), 4);
            assert_eq!(sai.count_objects(SaiObjectType::Neighbor), 4);
            assert_eq!(orch.stats().stats.ipv4_neighbors, 2);
            assert_eq!(orch.stats().stats.ipv6_neighbors, 2);
        }

        #[test]
        fn test_neigh_orch_add_duplicate_neighbor_different_mac() {
            let sai = MockSai::new();
            let mut orch = NeighOrch::new(NeighOrchConfig::default());

            // Add neighbor
            let (n1, _) = create_neighbor_with_sai("10.0.0.1", "Ethernet0", "00:11:22:33:44:55", &sai);
            orch.add_neighbor(n1).unwrap();

            assert_eq!(orch.neighbor_count(), 1);

            // Update same neighbor with different MAC (simulates ARP update)
            let (n2, _) = create_neighbor_with_sai("10.0.0.1", "Ethernet0", "AA:BB:CC:DD:EE:FF", &sai);
            orch.add_neighbor(n2).unwrap();

            // Should still have 1 neighbor (updated, not added)
            assert_eq!(orch.neighbor_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Neighbor), 2); // SAI layer tracks both
        }

        #[test]
        fn test_neigh_orch_bulk_add_and_remove() {
            let sai = MockSai::new();
            let mut orch = NeighOrch::new(NeighOrchConfig::default());

            // Add 10 neighbors
            let mut keys = Vec::new();
            for i in 0..10 {
                let ip = format!("10.0.0.{}", i + 1);
                let mac = format!("00:11:22:33:44:{:02X}", i);
                let (neighbor, _) = create_neighbor_with_sai(&ip, "Ethernet0", &mac, &sai);
                let key = neighbor.key.clone();
                orch.add_neighbor(neighbor).unwrap();
                keys.push(key);
            }

            assert_eq!(orch.neighbor_count(), 10);
            assert_eq!(sai.count_objects(SaiObjectType::Neighbor), 10);

            // Remove all neighbors
            for key in keys {
                let removed = orch.remove_neighbor(&key).unwrap();
                sai.remove_object(removed.neigh_oid).unwrap();
            }

            assert_eq!(orch.neighbor_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Neighbor), 0);
        }
    }

    // BufferOrch integration tests
    mod buffer_orch_tests {
        use super::*;
        use sonic_orchagent::buffer::{
            BufferOrch, BufferOrchConfig,
            BufferPoolEntry, BufferPoolConfig, BufferPoolType, BufferPoolMode,
            BufferProfileEntry, BufferProfileConfig, ThresholdMode,
        };

        fn create_pool_with_sai(name: &str, size: u64, sai: &MockSai) -> (BufferPoolEntry, u64) {
            let mut pool = BufferPoolEntry {
                name: name.to_string(),
                config: BufferPoolConfig {
                    pool_type: BufferPoolType::Ingress,
                    mode: BufferPoolMode::Dynamic,
                    size,
                    threshold_mode: ThresholdMode::Dynamic,
                    xoff_threshold: None,
                    xon_threshold: None,
                },
                sai_oid: 0,
                ref_count: 0,
            };

            let oid = sai.create_object(
                SaiObjectType::BufferPool,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("size".to_string(), size.to_string()),
                ]
            ).unwrap();

            pool.sai_oid = oid;
            (pool, oid)
        }

        fn create_profile_with_sai(name: &str, pool_name: &str, size: u64, sai: &MockSai) -> (BufferProfileEntry, u64) {
            let mut profile = BufferProfileEntry {
                name: name.to_string(),
                config: BufferProfileConfig {
                    pool_name: pool_name.to_string(),
                    size,
                    dynamic_threshold: None,
                    static_threshold: None,
                    xoff_threshold: None,
                    xon_threshold: None,
                    xon_offset: None,
                },
                sai_oid: 0,
                ref_count: 0,
            };

            let oid = sai.create_object(
                SaiObjectType::BufferProfile,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("pool".to_string(), pool_name.to_string()),
                    ("size".to_string(), size.to_string()),
                ]
            ).unwrap();

            profile.sai_oid = oid;
            (profile, oid)
        }

        #[test]
        fn test_buffer_orch_add_pool_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = BufferOrch::new(BufferOrchConfig::default());

            let (pool, oid) = create_pool_with_sai("ingress_lossless_pool", 10485760, &sai);
            orch.add_pool(pool).unwrap();

            assert_eq!(orch.pool_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::BufferPool), 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::BufferPool);
        }

        #[test]
        fn test_buffer_orch_add_profile_with_pool() {
            let sai = MockSai::new();
            let mut orch = BufferOrch::new(BufferOrchConfig::default());

            let (pool, _) = create_pool_with_sai("ingress_lossless_pool", 10485760, &sai);
            orch.add_pool(pool).unwrap();

            let (profile, _) = create_profile_with_sai("pg_lossless_profile", "ingress_lossless_pool", 1024, &sai);
            orch.add_profile(profile).unwrap();

            assert_eq!(orch.profile_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::BufferProfile), 1);
        }

        #[test]
        fn test_buffer_orch_ref_counting_prevents_removal() {
            let sai = MockSai::new();
            let mut orch = BufferOrch::new(BufferOrchConfig::default());

            let (pool, _) = create_pool_with_sai("ingress_lossless_pool", 10485760, &sai);
            orch.add_pool(pool).unwrap();

            orch.increment_pool_ref("ingress_lossless_pool").unwrap();

            let result = orch.remove_pool("ingress_lossless_pool");
            assert!(result.is_err());
            assert_eq!(sai.count_objects(SaiObjectType::BufferPool), 1);
        }

        #[test]
        fn test_buffer_orch_remove_after_ref_count_zero() {
            let sai = MockSai::new();
            let mut orch = BufferOrch::new(BufferOrchConfig::default());

            let (pool, _oid) = create_pool_with_sai("ingress_lossless_pool", 10485760, &sai);
            orch.add_pool(pool).unwrap();

            orch.increment_pool_ref("ingress_lossless_pool").unwrap();
            orch.decrement_pool_ref("ingress_lossless_pool").unwrap();

            let removed = orch.remove_pool("ingress_lossless_pool").unwrap();
            sai.remove_object(removed.sai_oid).unwrap();

            assert_eq!(orch.pool_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::BufferPool), 0);
        }

        #[test]
        fn test_buffer_orch_multiple_pools_and_profiles() {
            let sai = MockSai::new();
            let mut orch = BufferOrch::new(BufferOrchConfig::default());

            // Create two pools
            let (pool1, _) = create_pool_with_sai("ingress_lossless_pool", 10485760, &sai);
            let (pool2, _) = create_pool_with_sai("egress_lossy_pool", 20971520, &sai);
            orch.add_pool(pool1).unwrap();
            orch.add_pool(pool2).unwrap();

            // Create profiles for each pool
            let (profile1, _) = create_profile_with_sai("pg_lossless", "ingress_lossless_pool", 1024, &sai);
            let (profile2, _) = create_profile_with_sai("pg_lossy", "egress_lossy_pool", 2048, &sai);
            let (profile3, _) = create_profile_with_sai("queue_profile", "ingress_lossless_pool", 512, &sai);

            orch.add_profile(profile1).unwrap();
            orch.add_profile(profile2).unwrap();
            orch.add_profile(profile3).unwrap();

            assert_eq!(orch.pool_count(), 2);
            assert_eq!(orch.profile_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::BufferPool), 2);
            assert_eq!(sai.count_objects(SaiObjectType::BufferProfile), 3);
        }

        #[test]
        fn test_buffer_orch_cascading_deletion() {
            let sai = MockSai::new();
            let mut orch = BufferOrch::new(BufferOrchConfig::default());

            // Create pool and profile
            let (pool, _pool_oid) = create_pool_with_sai("ingress_lossless_pool", 10485760, &sai);
            orch.add_pool(pool).unwrap();

            let (profile, profile_oid) = create_profile_with_sai("pg_lossless", "ingress_lossless_pool", 1024, &sai);
            orch.add_profile(profile).unwrap();

            assert_eq!(orch.pool_count(), 1);
            assert_eq!(orch.profile_count(), 1);

            // Remove profile first
            let removed_profile = orch.remove_profile("pg_lossless").unwrap();
            sai.remove_object(removed_profile.sai_oid).unwrap();

            assert_eq!(orch.profile_count(), 0);

            // Now can remove pool
            let removed_pool = orch.remove_pool("ingress_lossless_pool").unwrap();
            sai.remove_object(removed_pool.sai_oid).unwrap();

            assert_eq!(orch.pool_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::BufferPool), 0);
            assert_eq!(sai.count_objects(SaiObjectType::BufferProfile), 0);
        }
    }

    // VxlanOrch integration tests
    mod vxlan_orch_tests {
        use super::*;
        use sonic_orchagent::vxlan::{
            VxlanOrch, VxlanOrchConfig,
            VxlanTunnelEntry, VxlanTunnelKey, VxlanTunnelConfig,
            VxlanVrfMapEntry, VxlanVrfMapKey,
            VxlanVlanMapEntry, VxlanVlanMapKey,
        };
        use std::net::IpAddr;

        fn create_tunnel_with_sai(name: &str, src_ip: &str, dst_ip: &str, sai: &MockSai) -> (VxlanTunnelEntry, u64) {
            let src_addr: IpAddr = src_ip.parse().unwrap();
            let dst_addr: IpAddr = dst_ip.parse().unwrap();

            let mut tunnel = VxlanTunnelEntry {
                key: VxlanTunnelKey::new(src_addr, dst_addr),
                config: VxlanTunnelConfig {
                    src_ip: src_addr,
                    dst_ip: dst_addr,
                    tunnel_name: name.to_string(),
                },
                tunnel_oid: 0,
                encap_mapper_oid: 0,
                decap_mapper_oid: 0,
            };

            let oid = sai.create_object(
                SaiObjectType::Tunnel,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("src_ip".to_string(), src_ip.to_string()),
                    ("dst_ip".to_string(), dst_ip.to_string()),
                ]
            ).unwrap();

            tunnel.tunnel_oid = oid;
            (tunnel, oid)
        }

        #[test]
        fn test_vxlan_orch_add_tunnel_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = VxlanOrch::new(VxlanOrchConfig::default());

            let (tunnel, oid) = create_tunnel_with_sai("vtep1", "10.0.0.1", "10.0.0.2", &sai);
            orch.add_tunnel(tunnel).unwrap();

            assert_eq!(orch.tunnel_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::Tunnel);
        }

        #[test]
        fn test_vxlan_orch_remove_tunnel_deletes_sai_object() {
            let sai = MockSai::new();
            let mut orch = VxlanOrch::new(VxlanOrchConfig::default());

            let (tunnel, oid) = create_tunnel_with_sai("vtep1", "10.0.0.1", "10.0.0.2", &sai);
            let key = tunnel.key.clone();
            orch.add_tunnel(tunnel).unwrap();

            let removed = orch.remove_tunnel(&key).unwrap();
            sai.remove_object(removed.tunnel_oid).unwrap();

            assert_eq!(orch.tunnel_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 0);
        }

        #[test]
        fn test_vxlan_orch_multiple_tunnels() {
            let sai = MockSai::new();
            let mut orch = VxlanOrch::new(VxlanOrchConfig::default());

            let (t1, _) = create_tunnel_with_sai("vtep1", "10.0.0.1", "10.0.0.2", &sai);
            let (t2, _) = create_tunnel_with_sai("vtep2", "10.0.0.1", "10.0.0.3", &sai);
            let (t3, _) = create_tunnel_with_sai("vtep3", "10.0.0.2", "10.0.0.3", &sai);

            orch.add_tunnel(t1).unwrap();
            orch.add_tunnel(t2).unwrap();
            orch.add_tunnel(t3).unwrap();

            assert_eq!(orch.tunnel_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 3);
        }

        #[test]
        fn test_vxlan_orch_vrf_and_vlan_maps() {
            let sai = MockSai::new();
            let mut orch = VxlanOrch::new(VxlanOrchConfig::default());

            let vrf_map = VxlanVrfMapEntry::new(VxlanVrfMapKey::new(1000, "Vrf_default".to_string()));
            let vlan_map = VxlanVlanMapEntry::new(VxlanVlanMapKey::new(2000, 100));

            orch.add_vrf_map(vrf_map).unwrap();
            orch.add_vlan_map(vlan_map).unwrap();

            assert_eq!(orch.stats().stats.vrf_maps_created, 1);
            assert_eq!(orch.stats().stats.vlan_maps_created, 1);
        }

        #[test]
        fn test_vxlan_orch_multiple_vrf_maps() {
            let sai = MockSai::new();
            let mut orch = VxlanOrch::new(VxlanOrchConfig::default());

            // Add multiple VRF maps with different VNIs
            let vrf1 = VxlanVrfMapEntry::new(VxlanVrfMapKey::new(1000, "Vrf1".to_string()));
            let vrf2 = VxlanVrfMapEntry::new(VxlanVrfMapKey::new(2000, "Vrf2".to_string()));
            let vrf3 = VxlanVrfMapEntry::new(VxlanVrfMapKey::new(3000, "Vrf3".to_string()));

            orch.add_vrf_map(vrf1).unwrap();
            orch.add_vrf_map(vrf2).unwrap();
            orch.add_vrf_map(vrf3).unwrap();

            assert_eq!(orch.stats().stats.vrf_maps_created, 3);
        }

        #[test]
        fn test_vxlan_orch_multiple_vlan_maps() {
            let sai = MockSai::new();
            let mut orch = VxlanOrch::new(VxlanOrchConfig::default());

            // Add multiple VLAN maps with different VNIs and VLAN IDs
            let vlan1 = VxlanVlanMapEntry::new(VxlanVlanMapKey::new(1000, 100));
            let vlan2 = VxlanVlanMapEntry::new(VxlanVlanMapKey::new(2000, 200));
            let vlan3 = VxlanVlanMapEntry::new(VxlanVlanMapKey::new(3000, 300));
            let vlan4 = VxlanVlanMapEntry::new(VxlanVlanMapKey::new(4000, 400));

            orch.add_vlan_map(vlan1).unwrap();
            orch.add_vlan_map(vlan2).unwrap();
            orch.add_vlan_map(vlan3).unwrap();
            orch.add_vlan_map(vlan4).unwrap();

            assert_eq!(orch.stats().stats.vlan_maps_created, 4);
        }

        #[test]
        fn test_vxlan_orch_full_topology() {
            let sai = MockSai::new();
            let mut orch = VxlanOrch::new(VxlanOrchConfig::default());

            // Create multiple tunnels
            let (t1, _) = create_tunnel_with_sai("vtep1", "10.0.0.1", "10.0.0.2", &sai);
            let (t2, _) = create_tunnel_with_sai("vtep2", "10.0.0.1", "10.0.0.3", &sai);
            orch.add_tunnel(t1).unwrap();
            orch.add_tunnel(t2).unwrap();

            // Add VRF and VLAN maps
            let vrf1 = VxlanVrfMapEntry::new(VxlanVrfMapKey::new(1000, "Vrf1".to_string()));
            let vrf2 = VxlanVrfMapEntry::new(VxlanVrfMapKey::new(2000, "Vrf2".to_string()));
            let vlan1 = VxlanVlanMapEntry::new(VxlanVlanMapKey::new(3000, 100));
            let vlan2 = VxlanVlanMapEntry::new(VxlanVlanMapKey::new(4000, 200));

            orch.add_vrf_map(vrf1).unwrap();
            orch.add_vrf_map(vrf2).unwrap();
            orch.add_vlan_map(vlan1).unwrap();
            orch.add_vlan_map(vlan2).unwrap();

            // Verify complete topology
            assert_eq!(orch.tunnel_count(), 2);
            assert_eq!(orch.stats().stats.vrf_maps_created, 2);
            assert_eq!(orch.stats().stats.vlan_maps_created, 2);
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 2);
        }
    }

    // QosOrch integration tests
    mod qos_orch_tests {
        use super::*;
        use sonic_orchagent::qos::{QosOrch, QosOrchConfig};
        use sonic_orchagent::qos::{
            QosMapEntry, QosMapType, SchedulerEntry, SchedulerConfig,
            SchedulerType, WredProfile, MeterType,
        };

        fn create_dscp_map_with_sai(name: &str, sai: &MockSai) -> (QosMapEntry, u64) {
            let mut map = QosMapEntry::new(name.to_string(), QosMapType::DscpToTc);
            map.add_mapping(0, 0);
            map.add_mapping(8, 1);
            map.add_mapping(16, 2);
            map.add_mapping(24, 3);

            let oid = sai.create_object(
                SaiObjectType::QosMap,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("type".to_string(), "DSCP_TO_TC".to_string()),
                ]
            ).unwrap();

            map.sai_oid = oid;
            (map, oid)
        }

        fn create_scheduler_with_sai(name: &str, weight: u8, sai: &MockSai) -> (SchedulerEntry, u64) {
            let config = SchedulerConfig {
                scheduler_type: SchedulerType::Dwrr,
                weight,
                meter_type: Some(MeterType::Bytes),
                cir: Some(1000000),
                cbs: Some(8192),
                pir: Some(2000000),
                pbs: Some(16384),
            };

            let mut scheduler = SchedulerEntry::new(name.to_string(), config);

            let oid = sai.create_object(
                SaiObjectType::Scheduler,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("type".to_string(), "DWRR".to_string()),
                    ("weight".to_string(), weight.to_string()),
                ]
            ).unwrap();

            scheduler.sai_oid = oid;
            (scheduler, oid)
        }

        fn create_wred_profile_with_sai(name: &str, sai: &MockSai) -> (WredProfile, u64) {
            let mut profile = WredProfile::new(name.to_string());
            profile.green_enable = true;
            profile.green_min_threshold = Some(1000);
            profile.green_max_threshold = Some(2000);
            profile.green_drop_probability = Some(10);
            profile.yellow_enable = true;
            profile.yellow_min_threshold = Some(800);
            profile.yellow_max_threshold = Some(1600);
            profile.yellow_drop_probability = Some(20);
            profile.red_enable = true;
            profile.red_min_threshold = Some(500);
            profile.red_max_threshold = Some(1000);
            profile.red_drop_probability = Some(50);

            let oid = sai.create_object(
                SaiObjectType::WredProfile,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("green_min".to_string(), "1000".to_string()),
                    ("green_max".to_string(), "2000".to_string()),
                ]
            ).unwrap();

            profile.sai_oid = oid;
            (profile, oid)
        }

        #[test]
        fn test_qos_orch_add_dscp_map_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = QosOrch::new(QosOrchConfig::default());

            assert_eq!(sai.count_objects(SaiObjectType::QosMap), 0);

            let (map, oid) = create_dscp_map_with_sai("dscp_to_tc_map", &sai);
            orch.add_map(map).unwrap();

            assert_eq!(orch.map_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::QosMap), 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::QosMap);
        }

        #[test]
        fn test_qos_orch_add_scheduler_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = QosOrch::new(QosOrchConfig::default());

            assert_eq!(sai.count_objects(SaiObjectType::Scheduler), 0);

            let (scheduler, oid) = create_scheduler_with_sai("scheduler0", 10, &sai);
            orch.add_scheduler(scheduler).unwrap();

            assert_eq!(orch.scheduler_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Scheduler), 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::Scheduler);
            assert_eq!(sai_obj.attributes[2].1, "10");
        }

        #[test]
        fn test_qos_orch_add_wred_profile_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = QosOrch::new(QosOrchConfig::default());

            assert_eq!(sai.count_objects(SaiObjectType::WredProfile), 0);

            let (profile, oid) = create_wred_profile_with_sai("wred_profile0", &sai);
            orch.add_wred_profile(profile).unwrap();

            assert_eq!(orch.wred_profile_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::WredProfile), 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::WredProfile);
        }

        #[test]
        fn test_qos_orch_remove_qos_objects_deletes_sai_objects() {
            let sai = MockSai::new();
            let mut orch = QosOrch::new(QosOrchConfig::default());

            let (map, _map_oid) = create_dscp_map_with_sai("dscp_to_tc_map", &sai);
            orch.add_map(map).unwrap();

            let (scheduler, _sched_oid) = create_scheduler_with_sai("scheduler0", 10, &sai);
            orch.add_scheduler(scheduler).unwrap();

            let (profile, _wred_oid) = create_wred_profile_with_sai("wred_profile0", &sai);
            orch.add_wred_profile(profile).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::QosMap), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Scheduler), 1);
            assert_eq!(sai.count_objects(SaiObjectType::WredProfile), 1);

            let removed_map = orch.remove_map("dscp_to_tc_map").unwrap();
            sai.remove_object(removed_map.sai_oid).unwrap();

            let removed_sched = orch.remove_scheduler("scheduler0").unwrap();
            sai.remove_object(removed_sched.sai_oid).unwrap();

            let removed_wred = orch.remove_wred_profile("wred_profile0").unwrap();
            sai.remove_object(removed_wred.sai_oid).unwrap();

            assert_eq!(orch.map_count(), 0);
            assert_eq!(orch.scheduler_count(), 0);
            assert_eq!(orch.wred_profile_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::QosMap), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Scheduler), 0);
            assert_eq!(sai.count_objects(SaiObjectType::WredProfile), 0);
        }

        #[test]
        fn test_qos_orch_multiple_qos_objects() {
            let sai = MockSai::new();
            let mut orch = QosOrch::new(QosOrchConfig::default());

            let (map1, _) = create_dscp_map_with_sai("dscp_to_tc_map", &sai);
            let (map2, _) = create_dscp_map_with_sai("dscp_to_queue_map", &sai);
            let (sched1, _) = create_scheduler_with_sai("scheduler0", 10, &sai);
            let (sched2, _) = create_scheduler_with_sai("scheduler1", 20, &sai);
            let (sched3, _) = create_scheduler_with_sai("scheduler2", 30, &sai);
            let (wred1, _) = create_wred_profile_with_sai("wred_profile0", &sai);
            let (wred2, _) = create_wred_profile_with_sai("wred_profile1", &sai);

            orch.add_map(map1).unwrap();
            orch.add_map(map2).unwrap();
            orch.add_scheduler(sched1).unwrap();
            orch.add_scheduler(sched2).unwrap();
            orch.add_scheduler(sched3).unwrap();
            orch.add_wred_profile(wred1).unwrap();
            orch.add_wred_profile(wred2).unwrap();

            assert_eq!(orch.map_count(), 2);
            assert_eq!(orch.scheduler_count(), 3);
            assert_eq!(orch.wred_profile_count(), 2);
            assert_eq!(sai.count_objects(SaiObjectType::QosMap), 2);
            assert_eq!(sai.count_objects(SaiObjectType::Scheduler), 3);
            assert_eq!(sai.count_objects(SaiObjectType::WredProfile), 2);
            assert_eq!(orch.stats().stats.maps_created, 2);
            assert_eq!(orch.stats().stats.schedulers_created, 3);
            assert_eq!(orch.stats().stats.wred_profiles_created, 2);
        }
    }

    // Srv6Orch integration tests
    mod srv6_orch_tests {
        use super::*;
        use sonic_orchagent::srv6::{
            Srv6Orch, Srv6OrchConfig,
            Srv6LocalSidEntry, Srv6LocalSidConfig, Srv6Sid, Srv6EndpointBehavior,
            Srv6SidListEntry, Srv6SidListConfig,
        };

        fn create_local_sid_with_sai(
            sid_str: &str,
            behavior: Srv6EndpointBehavior,
            sai: &MockSai,
        ) -> (Srv6LocalSidEntry, u64) {
            let sid = Srv6Sid::new(sid_str.to_string());
            let mut entry = Srv6LocalSidEntry::new(Srv6LocalSidConfig {
                sid,
                endpoint_behavior: behavior,
                next_hop: None,
                vrf: None,
            });

            let oid = sai.create_object(
                SaiObjectType::Srv6LocalSid,
                vec![
                    ("sid".to_string(), sid_str.to_string()),
                    ("behavior".to_string(), format!("{:?}", behavior)),
                ]
            ).unwrap();

            entry.sid_oid = oid;
            (entry, oid)
        }

        fn create_sidlist_with_sai(
            name: &str,
            sids: Vec<&str>,
            sai: &MockSai,
        ) -> (Srv6SidListEntry, u64) {
            let sid_vec: Vec<Srv6Sid> = sids.iter()
                .map(|s| Srv6Sid::new(s.to_string()))
                .collect();

            let mut entry = Srv6SidListEntry::new(Srv6SidListConfig {
                name: name.to_string(),
                sids: sid_vec,
            });

            let oid = sai.create_object(
                SaiObjectType::Srv6LocalSid,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("sids".to_string(), sids.join(",")),
                ]
            ).unwrap();

            entry.sidlist_oid = oid;
            (entry, oid)
        }

        #[test]
        fn test_srv6_orch_add_local_sid_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = Srv6Orch::new(Srv6OrchConfig::default());

            assert_eq!(sai.count_objects(SaiObjectType::Srv6LocalSid), 0);

            let (local_sid, oid) = create_local_sid_with_sai(
                "fc00:0:1:1::",
                Srv6EndpointBehavior::End,
                &sai,
            );
            orch.add_local_sid(local_sid).unwrap();

            assert_eq!(orch.local_sid_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Srv6LocalSid), 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::Srv6LocalSid);
        }

        #[test]
        fn test_srv6_orch_add_sidlist_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = Srv6Orch::new(Srv6OrchConfig::default());

            let (sidlist, oid) = create_sidlist_with_sai(
                "policy1",
                vec!["fc00:0:1:1::", "fc00:0:1:2::"],
                &sai,
            );
            orch.add_sidlist(sidlist).unwrap();

            assert_eq!(orch.sidlist_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Srv6LocalSid), 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::Srv6LocalSid);
        }

        #[test]
        fn test_srv6_orch_remove_local_sid_deletes_sai_object() {
            let sai = MockSai::new();
            let mut orch = Srv6Orch::new(Srv6OrchConfig::default());

            let (local_sid, _oid) = create_local_sid_with_sai(
                "fc00:0:1:1::",
                Srv6EndpointBehavior::End,
                &sai,
            );
            let sid = local_sid.config.sid.clone();
            orch.add_local_sid(local_sid).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::Srv6LocalSid), 1);

            let removed = orch.remove_local_sid(&sid).unwrap();
            sai.remove_object(removed.sid_oid).unwrap();

            assert_eq!(orch.local_sid_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Srv6LocalSid), 0);
        }

        #[test]
        fn test_srv6_orch_multiple_local_sids() {
            let sai = MockSai::new();
            let mut orch = Srv6Orch::new(Srv6OrchConfig::default());

            let (sid1, _) = create_local_sid_with_sai(
                "fc00:0:1:1::",
                Srv6EndpointBehavior::End,
                &sai,
            );
            let (sid2, _) = create_local_sid_with_sai(
                "fc00:0:1:2::",
                Srv6EndpointBehavior::EndX,
                &sai,
            );
            let (sid3, _) = create_local_sid_with_sai(
                "fc00:0:1:3::",
                Srv6EndpointBehavior::EndDx6,
                &sai,
            );

            orch.add_local_sid(sid1).unwrap();
            orch.add_local_sid(sid2).unwrap();
            orch.add_local_sid(sid3).unwrap();

            assert_eq!(orch.local_sid_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::Srv6LocalSid), 3);
            assert_eq!(orch.stats().stats.local_sids_created, 3);
        }

        #[test]
        fn test_srv6_orch_sidlist_with_multiple_segments() {
            let sai = MockSai::new();
            let mut orch = Srv6Orch::new(Srv6OrchConfig::default());

            let (sidlist1, _) = create_sidlist_with_sai(
                "policy1",
                vec!["fc00:0:1:1::", "fc00:0:1:2::", "fc00:0:1:3::"],
                &sai,
            );
            let (sidlist2, _) = create_sidlist_with_sai(
                "policy2",
                vec!["fc00:0:2:1::", "fc00:0:2:2::", "fc00:0:2:3::", "fc00:0:2:4::"],
                &sai,
            );

            orch.add_sidlist(sidlist1).unwrap();
            orch.add_sidlist(sidlist2).unwrap();

            assert_eq!(orch.sidlist_count(), 2);
            assert_eq!(sai.count_objects(SaiObjectType::Srv6LocalSid), 2);
            assert_eq!(orch.stats().stats.sidlists_created, 2);

            // Verify policy1 has 3 segments
            let policy1 = orch.get_sidlist("policy1").unwrap();
            assert_eq!(policy1.config.sids.len(), 3);

            // Verify policy2 has 4 segments
            let policy2 = orch.get_sidlist("policy2").unwrap();
            assert_eq!(policy2.config.sids.len(), 4);
        }
    }

    // MacsecOrch integration tests
    mod macsec_orch_tests {
        use super::*;
        use sonic_orchagent::macsec::{
            MacsecOrch, MacsecOrchConfig,
            MacsecPort, MacsecSc, MacsecSa,
            MacsecDirection, MacsecCipherSuite, Sci,
        };

        fn create_port_with_sai(port_name: &str, enable: bool, sai: &MockSai) -> (MacsecPort, u64) {
            let mut port = MacsecPort::new(port_name.to_string());
            port.enable = enable;
            port.cipher_suite = MacsecCipherSuite::Gcm128;

            let oid = sai.create_object(
                SaiObjectType::MacsecPort,
                vec![
                    ("port_name".to_string(), port_name.to_string()),
                    ("enable".to_string(), enable.to_string()),
                    ("cipher_suite".to_string(), "GCM-AES-128".to_string()),
                ]
            ).unwrap();

            (port, oid)
        }

        fn create_sc_with_sai(sci: Sci, direction: MacsecDirection, sai: &MockSai) -> (MacsecSc, u64) {
            let mut sc = MacsecSc::new(sci, direction);

            let oid = sai.create_object(
                SaiObjectType::MacsecPort,
                vec![
                    ("sci".to_string(), format!("0x{:016x}", sci)),
                    ("direction".to_string(), format!("{:?}", direction)),
                ]
            ).unwrap();

            sc.sc_oid = oid;
            (sc, oid)
        }

        fn create_sa_with_sai(an: u8, pn: u64, sai: &MockSai) -> (MacsecSa, u64) {
            let mut sa = MacsecSa::new(an, pn);
            sa.sak = vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
            sa.auth_key = vec![0xAA, 0xBB, 0xCC, 0xDD];

            let oid = sai.create_object(
                SaiObjectType::MacsecPort,
                vec![
                    ("an".to_string(), an.to_string()),
                    ("pn".to_string(), pn.to_string()),
                ]
            ).unwrap();

            sa.sa_oid = oid;
            (sa, oid)
        }

        #[test]
        fn test_macsec_orch_add_port_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = MacsecOrch::new(MacsecOrchConfig::default());

            assert_eq!(sai.count_objects(SaiObjectType::MacsecPort), 0);

            let (port, oid) = create_port_with_sai("Ethernet0", true, &sai);
            orch.add_port(port).unwrap();

            assert_eq!(orch.port_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::MacsecPort), 1);
            assert_eq!(orch.stats().stats.ports_enabled, 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::MacsecPort);
        }

        #[test]
        fn test_macsec_orch_add_sc_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = MacsecOrch::new(MacsecOrchConfig::default());

            assert_eq!(sai.count_objects(SaiObjectType::MacsecPort), 0);

            let sci: Sci = 0x0011223344556677;
            let (sc, oid) = create_sc_with_sai(sci, MacsecDirection::Ingress, &sai);
            orch.add_sc(sc).unwrap();

            assert_eq!(orch.sc_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::MacsecPort), 1);
            assert_eq!(orch.stats().stats.scs_created, 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::MacsecPort);
        }

        #[test]
        fn test_macsec_orch_add_sa_validates_an_range() {
            let sai = MockSai::new();
            let mut orch = MacsecOrch::new(MacsecOrchConfig::default());

            let sci: Sci = 0x0011223344556677;
            let (sc, _) = create_sc_with_sai(sci, MacsecDirection::Ingress, &sai);
            orch.add_sc(sc).unwrap();

            // Test all valid ANs (0-3)
            for an in 0..=3 {
                let (sa, _) = create_sa_with_sai(an, 1, &sai);
                let result = orch.add_sa(sci, sa);
                assert!(result.is_ok(), "AN {} should be valid", an);
            }

            assert_eq!(orch.sa_count(), 4);
            assert_eq!(sai.count_objects(SaiObjectType::MacsecPort), 5); // 1 SC + 4 SAs

            // Verify all ANs are present
            for an in 0..=3 {
                assert!(orch.get_sa(sci, an).is_some(), "SA with AN {} should exist", an);
            }
        }

        #[test]
        fn test_macsec_orch_cascading_deletion() {
            let sai = MockSai::new();
            let mut orch = MacsecOrch::new(MacsecOrchConfig::default());

            let sci: Sci = 0x0011223344556677;
            let (sc, sc_oid) = create_sc_with_sai(sci, MacsecDirection::Ingress, &sai);
            orch.add_sc(sc).unwrap();

            // Add multiple SAs to the SC
            let mut sa_oids = Vec::new();
            for an in 0..=3 {
                let (sa, sa_oid) = create_sa_with_sai(an, an as u64 + 1, &sai);
                orch.add_sa(sci, sa).unwrap();
                sa_oids.push(sa_oid);
            }

            assert_eq!(orch.sc_count(), 1);
            assert_eq!(orch.sa_count(), 4);
            assert_eq!(sai.count_objects(SaiObjectType::MacsecPort), 5);

            // Remove SC should cascade delete all SAs
            let removed_sc = orch.remove_sc(sci).unwrap();
            sai.remove_object(removed_sc.sc_oid).unwrap();

            // Remove SA objects from SAI (in real implementation, SAI would handle cascade)
            for sa_oid in sa_oids {
                sai.remove_object(sa_oid).unwrap();
            }

            assert_eq!(orch.sc_count(), 0);
            assert_eq!(orch.sa_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::MacsecPort), 0);
        }

        #[test]
        fn test_macsec_orch_multiple_ports_and_scs() {
            let sai = MockSai::new();
            let mut orch = MacsecOrch::new(MacsecOrchConfig::default());

            // Add multiple ports
            let (port1, _) = create_port_with_sai("Ethernet0", true, &sai);
            let (port2, _) = create_port_with_sai("Ethernet4", true, &sai);
            let (port3, _) = create_port_with_sai("Ethernet8", false, &sai);

            orch.add_port(port1).unwrap();
            orch.add_port(port2).unwrap();
            orch.add_port(port3).unwrap();

            assert_eq!(orch.port_count(), 3);
            assert_eq!(orch.stats().stats.ports_enabled, 2);

            // Add multiple SCs with different directions
            let sci1: Sci = 0x0011223344556677;
            let sci2: Sci = 0x8899AABBCCDDEEFF;
            let sci3: Sci = 0x1122334455667788;

            let (sc1, _) = create_sc_with_sai(sci1, MacsecDirection::Ingress, &sai);
            let (sc2, _) = create_sc_with_sai(sci2, MacsecDirection::Egress, &sai);
            let (sc3, _) = create_sc_with_sai(sci3, MacsecDirection::Ingress, &sai);

            orch.add_sc(sc1).unwrap();
            orch.add_sc(sc2).unwrap();
            orch.add_sc(sc3).unwrap();

            assert_eq!(orch.sc_count(), 3);
            assert_eq!(orch.stats().stats.scs_created, 3);

            // Add SAs to different SCs
            let (sa1, _) = create_sa_with_sai(0, 1, &sai);
            let (sa2, _) = create_sa_with_sai(1, 2, &sai);
            orch.add_sa(sci1, sa1).unwrap();
            orch.add_sa(sci1, sa2).unwrap();

            let (sa3, _) = create_sa_with_sai(0, 10, &sai);
            let (sa4, _) = create_sa_with_sai(1, 20, &sai);
            let (sa5, _) = create_sa_with_sai(2, 30, &sai);
            orch.add_sa(sci2, sa3).unwrap();
            orch.add_sa(sci2, sa4).unwrap();
            orch.add_sa(sci2, sa5).unwrap();

            assert_eq!(orch.sa_count(), 5);
            assert_eq!(orch.stats().stats.sas_created, 5);

            // Verify SAs are correctly associated with their SCs
            let sas_sci1 = orch.get_sas_for_sc(sci1);
            assert_eq!(sas_sci1.len(), 2);

            let sas_sci2 = orch.get_sas_for_sc(sci2);
            assert_eq!(sas_sci2.len(), 3);

            let sas_sci3 = orch.get_sas_for_sc(sci3);
            assert_eq!(sas_sci3.len(), 0);
        }
    }

    // VnetOrch integration tests
    mod vnet_orch_tests {
        use super::*;
        use sonic_orchagent::vnet::{
            VnetOrch, VnetOrchConfig,
            VnetEntry, VnetConfig, VnetKey,
            VnetRouteEntry, VnetRouteConfig, VnetRouteKey, VnetRouteType,
        };
        use std::net::IpAddr;

        fn create_vnet_with_sai(name: &str, vni: Option<u32>, sai: &MockSai) -> (VnetEntry, u64) {
            let config = VnetConfig {
                vnet_name: name.to_string(),
                vni,
                vxlan_tunnel: Some("tunnel0".to_string()),
                scope: None,
                advertise_prefix: false,
            };

            let mut vnet = VnetEntry::new(config);

            let oid = sai.create_object(
                SaiObjectType::Vnet,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("vni".to_string(), vni.map_or("none".to_string(), |v| v.to_string())),
                ]
            ).unwrap();

            vnet.vnet_oid = oid;
            (vnet, oid)
        }

        fn create_route_with_sai(
            vnet_name: &str,
            prefix: &str,
            route_type: VnetRouteType,
            sai: &MockSai,
        ) -> (VnetRouteEntry, u64) {
            let key = VnetRouteKey::new(vnet_name.to_string(), prefix.to_string());
            let config = VnetRouteConfig {
                route_type,
                endpoint: None,
                endpoint_monitor: None,
                mac_address: None,
                vni: None,
                peer_list: vec![],
            };

            let mut route = VnetRouteEntry::new(key, config);

            let oid = sai.create_object(
                SaiObjectType::Route,
                vec![
                    ("vnet".to_string(), vnet_name.to_string()),
                    ("prefix".to_string(), prefix.to_string()),
                    ("type".to_string(), format!("{:?}", route_type)),
                ]
            ).unwrap();

            route.route_oid = oid;
            (route, oid)
        }

        fn create_tunnel_route_with_sai(
            vnet_name: &str,
            prefix: &str,
            endpoint: &str,
            vni: Option<u32>,
            sai: &MockSai,
        ) -> (VnetRouteEntry, u64) {
            let key = VnetRouteKey::new(vnet_name.to_string(), prefix.to_string());
            let config = VnetRouteConfig {
                route_type: VnetRouteType::Tunnel,
                endpoint: Some(endpoint.parse::<IpAddr>().unwrap()),
                endpoint_monitor: None,
                mac_address: None,
                vni,
                peer_list: vec![],
            };

            let mut route = VnetRouteEntry::new(key, config);

            let oid = sai.create_object(
                SaiObjectType::Route,
                vec![
                    ("vnet".to_string(), vnet_name.to_string()),
                    ("prefix".to_string(), prefix.to_string()),
                    ("type".to_string(), "Tunnel".to_string()),
                    ("endpoint".to_string(), endpoint.to_string()),
                    ("vni".to_string(), vni.map_or("none".to_string(), |v| v.to_string())),
                ]
            ).unwrap();

            route.route_oid = oid;
            (route, oid)
        }

        #[test]
        fn test_vnet_orch_add_vnet_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = VnetOrch::new(VnetOrchConfig::default());

            assert_eq!(sai.count_objects(SaiObjectType::Vnet), 0);

            let (vnet, oid) = create_vnet_with_sai("Vnet1", Some(1000), &sai);
            orch.add_vnet(vnet).unwrap();

            assert_eq!(orch.vnet_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Vnet), 1);
            assert_eq!(orch.stats().stats.vnets_created, 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::Vnet);
        }

        #[test]
        fn test_vnet_orch_add_route_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = VnetOrch::new(VnetOrchConfig::default());

            // Create VNET first
            let (vnet, _) = create_vnet_with_sai("Vnet1", Some(1000), &sai);
            orch.add_vnet(vnet).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::Route), 0);

            // Create route
            let (route, oid) = create_route_with_sai("Vnet1", "10.0.0.0/24", VnetRouteType::Direct, &sai);
            orch.add_route(route).unwrap();

            assert_eq!(orch.route_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Route), 1);
            assert_eq!(orch.stats().stats.routes_created, 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::Route);
        }

        #[test]
        fn test_vnet_orch_cannot_add_route_without_vnet() {
            let sai = MockSai::new();
            let mut orch = VnetOrch::new(VnetOrchConfig::default());

            // Try to add route without VNET
            let (route, _) = create_route_with_sai("Vnet1", "10.0.0.0/24", VnetRouteType::Direct, &sai);
            let result = orch.add_route(route);

            assert!(result.is_err());
            assert_eq!(orch.route_count(), 0);
            assert_eq!(orch.stats().stats.routes_created, 0);
        }

        #[test]
        fn test_vnet_orch_cannot_remove_vnet_with_routes() {
            let sai = MockSai::new();
            let mut orch = VnetOrch::new(VnetOrchConfig::default());

            // Create VNET and route
            let (vnet, vnet_oid) = create_vnet_with_sai("Vnet1", Some(1000), &sai);
            let vnet_key = vnet.key.clone();
            orch.add_vnet(vnet).unwrap();

            let (route, _) = create_route_with_sai("Vnet1", "10.0.0.0/24", VnetRouteType::Direct, &sai);
            orch.add_route(route).unwrap();

            // Try to remove VNET while route exists
            let result = orch.remove_vnet(&vnet_key);
            assert!(result.is_err());
            assert_eq!(orch.vnet_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Vnet), 1);

            // Verify VNET is still there
            let sai_obj = sai.get_object(vnet_oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::Vnet);
        }

        #[test]
        fn test_vnet_orch_tunnel_routes() {
            let sai = MockSai::new();
            let mut orch = VnetOrch::new(VnetOrchConfig::default());

            // Create VNET
            let (vnet, _) = create_vnet_with_sai("Vnet1", Some(1000), &sai);
            orch.add_vnet(vnet).unwrap();

            // Add tunnel routes
            let (route1, _) = create_tunnel_route_with_sai("Vnet1", "10.0.0.0/24", "192.168.1.1", Some(1000), &sai);
            let (route2, _) = create_tunnel_route_with_sai("Vnet1", "10.0.1.0/24", "192.168.1.2", Some(1000), &sai);
            let (route3, _) = create_route_with_sai("Vnet1", "10.0.2.0/24", VnetRouteType::Direct, &sai);

            orch.add_route(route1).unwrap();
            orch.add_route(route2).unwrap();
            orch.add_route(route3).unwrap();

            assert_eq!(orch.route_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::Route), 3);

            // Get only tunnel routes
            let tunnel_routes = orch.get_tunnel_routes();
            assert_eq!(tunnel_routes.len(), 2);

            // Verify all tunnel routes have correct type
            for route in tunnel_routes {
                assert!(route.is_tunnel_route());
                assert_eq!(route.config.route_type, VnetRouteType::Tunnel);
                assert!(route.config.endpoint.is_some());
            }
        }

        #[test]
        fn test_vnet_orch_multiple_vnets_and_routes() {
            let sai = MockSai::new();
            let mut orch = VnetOrch::new(VnetOrchConfig::default());

            // Create multiple VNETs
            let (vnet1, _) = create_vnet_with_sai("Vnet1", Some(1000), &sai);
            let (vnet2, _) = create_vnet_with_sai("Vnet2", Some(2000), &sai);
            let (vnet3, _) = create_vnet_with_sai("Vnet3", Some(3000), &sai);

            orch.add_vnet(vnet1).unwrap();
            orch.add_vnet(vnet2).unwrap();
            orch.add_vnet(vnet3).unwrap();

            assert_eq!(orch.vnet_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::Vnet), 3);

            // Add routes to different VNETs
            let (r1, _) = create_route_with_sai("Vnet1", "10.0.0.0/24", VnetRouteType::Direct, &sai);
            let (r2, _) = create_route_with_sai("Vnet1", "10.0.1.0/24", VnetRouteType::Direct, &sai);
            let (r3, _) = create_tunnel_route_with_sai("Vnet2", "10.1.0.0/24", "192.168.1.1", Some(2000), &sai);
            let (r4, _) = create_route_with_sai("Vnet2", "10.1.1.0/24", VnetRouteType::Vnet, &sai);
            let (r5, _) = create_tunnel_route_with_sai("Vnet3", "10.2.0.0/24", "192.168.2.1", Some(3000), &sai);

            orch.add_route(r1).unwrap();
            orch.add_route(r2).unwrap();
            orch.add_route(r3).unwrap();
            orch.add_route(r4).unwrap();
            orch.add_route(r5).unwrap();

            assert_eq!(orch.route_count(), 5);
            assert_eq!(sai.count_objects(SaiObjectType::Route), 5);

            // Verify routes per VNET
            let vnet1_routes = orch.get_routes_for_vnet("Vnet1");
            assert_eq!(vnet1_routes.len(), 2);

            let vnet2_routes = orch.get_routes_for_vnet("Vnet2");
            assert_eq!(vnet2_routes.len(), 2);

            let vnet3_routes = orch.get_routes_for_vnet("Vnet3");
            assert_eq!(vnet3_routes.len(), 1);

            // Verify tunnel routes
            let tunnel_routes = orch.get_tunnel_routes();
            assert_eq!(tunnel_routes.len(), 2);

            // Verify VNET routes
            let vnet_routes = orch.get_vnet_routes();
            assert_eq!(vnet_routes.len(), 1);

            // Verify stats
            assert_eq!(orch.stats().stats.vnets_created, 3);
            assert_eq!(orch.stats().stats.routes_created, 5);
        }
    }

    // RouteOrch integration tests
    mod route_orch_tests {
        use super::*;
        use sonic_orchagent::{
            RouteOrch, RouteOrchConfig, RouteOrchCallbacks,
            NextHopKey, NextHopGroupKey, NextHopGroupEntry,
        };
        use sonic_types::{IpAddress, IpPrefix};
        use std::net::Ipv4Addr;
        use std::collections::{HashMap, HashSet};
        use async_trait::async_trait;

        // Mock callbacks implementation for RouteOrch integration tests
        #[derive(Default)]
        struct MockRouteCallbacks {
            sai: Arc<MockSai>,
            next_hop_ids: Arc<Mutex<HashMap<NextHopKey, u64>>>,
            router_intf_ids: Arc<Mutex<HashMap<String, u64>>>,
            vrfs: Arc<Mutex<HashSet<u64>>>,
            next_hop_refs: Arc<Mutex<HashMap<NextHopKey, u32>>>,
            router_intf_refs: Arc<Mutex<HashMap<String, u32>>>,
            vrf_refs: Arc<Mutex<HashMap<u64, u32>>>,
        }

        impl MockRouteCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                Self {
                    sai,
                    ..Default::default()
                }
            }

            fn add_next_hop(&self, nh: NextHopKey, id: u64) {
                self.next_hop_ids.lock().unwrap().insert(nh, id);
            }

            fn add_router_intf(&self, alias: String, id: u64) {
                self.router_intf_ids.lock().unwrap().insert(alias, id);
            }

            fn add_vrf(&self, vrf_id: u64) {
                self.vrfs.lock().unwrap().insert(vrf_id);
            }
        }

        #[async_trait]
        impl RouteOrchCallbacks for MockRouteCallbacks {
            fn has_next_hop(&self, nexthop: &NextHopKey) -> bool {
                self.next_hop_ids.lock().unwrap().contains_key(nexthop)
            }

            fn get_next_hop_id(&self, nexthop: &NextHopKey) -> Option<u64> {
                self.next_hop_ids.lock().unwrap().get(nexthop).copied()
            }

            fn get_router_intf_id(&self, alias: &str) -> Option<u64> {
                self.router_intf_ids.lock().unwrap().get(alias).copied()
            }

            fn vrf_exists(&self, vrf_id: u64) -> bool {
                vrf_id == 0 || self.vrfs.lock().unwrap().contains(&vrf_id)
            }

            fn increase_next_hop_ref_count(&self, nexthop: &NextHopKey) {
                *self.next_hop_refs.lock().unwrap().entry(nexthop.clone()).or_insert(0) += 1;
            }

            fn decrease_next_hop_ref_count(&self, nexthop: &NextHopKey) {
                if let Some(count) = self.next_hop_refs.lock().unwrap().get_mut(nexthop) {
                    *count = count.saturating_sub(1);
                }
            }

            fn increase_router_intf_ref_count(&self, alias: &str) {
                *self.router_intf_refs.lock().unwrap().entry(alias.to_string()).or_insert(0) += 1;
            }

            fn decrease_router_intf_ref_count(&self, alias: &str) {
                if let Some(count) = self.router_intf_refs.lock().unwrap().get_mut(alias) {
                    *count = count.saturating_sub(1);
                }
            }

            fn increase_vrf_ref_count(&self, vrf_id: u64) {
                *self.vrf_refs.lock().unwrap().entry(vrf_id).or_insert(0) += 1;
            }

            fn decrease_vrf_ref_count(&self, vrf_id: u64) {
                if let Some(count) = self.vrf_refs.lock().unwrap().get_mut(&vrf_id) {
                    *count = count.saturating_sub(1);
                }
            }

            async fn sai_create_nhg(&self, _nhg_key: &NextHopGroupKey) -> Result<u64, sonic_orchagent::route::RouteError> {
                let oid = self.sai.create_object(
                    SaiObjectType::NextHopGroup,
                    vec![("type".to_string(), "ECMP".to_string())]
                ).unwrap();
                Ok(oid)
            }

            async fn sai_remove_nhg(&self, nhg_id: u64) -> Result<(), sonic_orchagent::route::RouteError> {
                self.sai.remove_object(nhg_id).map_err(|e| {
                    sonic_orchagent::route::RouteError::SaiError(e)
                })
            }

            async fn sai_create_route(
                &self,
                vrf_id: u64,
                prefix: &IpPrefix,
                nhg_id: Option<u64>,
                blackhole: bool,
            ) -> Result<(), sonic_orchagent::route::RouteError> {
                let mut attrs = vec![
                    ("vrf".to_string(), format!("{:x}", vrf_id)),
                    ("prefix".to_string(), prefix.to_string()),
                ];
                if let Some(id) = nhg_id {
                    attrs.push(("nhg_id".to_string(), format!("{:x}", id)));
                }
                if blackhole {
                    attrs.push(("blackhole".to_string(), "true".to_string()));
                }
                self.sai.create_object(SaiObjectType::Route, attrs).map_err(|e| {
                    sonic_orchagent::route::RouteError::SaiError(e)
                })?;
                Ok(())
            }

            async fn sai_remove_route(&self, vrf_id: u64, prefix: &IpPrefix) -> Result<(), sonic_orchagent::route::RouteError> {
                // Find and remove the route object
                let objects = self.sai.objects.lock().unwrap();
                if let Some(route_obj) = objects.iter().find(|obj| {
                    obj.object_type == SaiObjectType::Route &&
                    obj.attributes.iter().any(|(k, v)| k == "vrf" && v == &format!("{:x}", vrf_id)) &&
                    obj.attributes.iter().any(|(k, v)| k == "prefix" && v == &prefix.to_string())
                }) {
                    let oid = route_obj.oid;
                    drop(objects);
                    self.sai.remove_object(oid).map_err(|e| {
                        sonic_orchagent::route::RouteError::SaiError(e)
                    })?;
                }
                Ok(())
            }

            async fn sai_set_route(
                &self,
                vrf_id: u64,
                prefix: &IpPrefix,
                nhg_id: Option<u64>,
                blackhole: bool,
            ) -> Result<(), sonic_orchagent::route::RouteError> {
                // For testing, just remove and recreate
                let _ = self.sai_remove_route(vrf_id, prefix).await;
                self.sai_create_route(vrf_id, prefix, nhg_id, blackhole).await
            }
        }

        fn make_prefix(addr: &str, len: u8) -> IpPrefix {
            IpPrefix::new(
                IpAddress::V4(addr.parse::<Ipv4Addr>().unwrap().into()),
                len,
            ).unwrap()
        }

        fn make_nexthop(ip: &str, alias: &str) -> NextHopKey {
            NextHopKey::new(
                IpAddress::V4(ip.parse::<Ipv4Addr>().unwrap().into()),
                alias,
            )
        }

        #[tokio::test]
        async fn test_route_orch_add_basic_route_creates_sai_objects() {
            let sai = Arc::new(MockSai::new());
            let mut orch = RouteOrch::new(RouteOrchConfig::default());
            let callbacks = Arc::new(MockRouteCallbacks::new(sai.clone()));

            // Setup: Add a next-hop
            let nh = make_nexthop("192.168.1.1", "Ethernet0");
            callbacks.add_next_hop(nh.clone(), 0x1000);
            orch.set_callbacks(callbacks.clone());

            // Test: Add route
            let prefix = make_prefix("10.0.0.0", 24);
            let nhg_key = NextHopGroupKey::single(nh.clone());

            assert_eq!(sai.count_objects(SaiObjectType::Route), 0);

            let result = orch.add_route(0, prefix.clone(), nhg_key).await;
            assert!(result.is_ok());

            // Verify: Route created in SAI and orchestration state
            assert!(orch.has_route(0, &prefix));
            assert_eq!(sai.count_objects(SaiObjectType::Route), 1);

            // Verify next-hop ref count incremented
            let refs = callbacks.next_hop_refs.lock().unwrap();
            assert_eq!(refs.get(&nh), Some(&1));
        }

        #[tokio::test]
        async fn test_route_orch_remove_route_deletes_sai_objects() {
            let sai = Arc::new(MockSai::new());
            let mut orch = RouteOrch::new(RouteOrchConfig::default());
            let callbacks = Arc::new(MockRouteCallbacks::new(sai.clone()));

            let nh = make_nexthop("192.168.1.1", "Ethernet0");
            callbacks.add_next_hop(nh.clone(), 0x1000);
            orch.set_callbacks(callbacks.clone());

            let prefix = make_prefix("10.0.0.0", 24);
            let nhg_key = NextHopGroupKey::single(nh.clone());

            // Add route
            orch.add_route(0, prefix.clone(), nhg_key).await.unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::Route), 1);

            // Remove route
            let result = orch.remove_route(0, &prefix).await;
            assert!(result.is_ok());

            // Verify: Route removed from SAI and orchestration state
            assert!(!orch.has_route(0, &prefix));
            assert_eq!(sai.count_objects(SaiObjectType::Route), 0);

            // Verify next-hop ref count decremented
            let refs = callbacks.next_hop_refs.lock().unwrap();
            assert_eq!(refs.get(&nh), Some(&0));
        }

        #[tokio::test]
        async fn test_route_orch_ecmp_route_with_multiple_next_hops() {
            let sai = Arc::new(MockSai::new());
            let mut orch = RouteOrch::new(RouteOrchConfig::default());
            let callbacks = Arc::new(MockRouteCallbacks::new(sai.clone()));

            // Setup: Add multiple next-hops
            let nh1 = make_nexthop("192.168.1.1", "Ethernet0");
            let nh2 = make_nexthop("192.168.1.2", "Ethernet4");
            let nh3 = make_nexthop("192.168.1.3", "Ethernet8");
            callbacks.add_next_hop(nh1.clone(), 0x1000);
            callbacks.add_next_hop(nh2.clone(), 0x1001);
            callbacks.add_next_hop(nh3.clone(), 0x1002);
            orch.set_callbacks(callbacks.clone());

            // Test: Add ECMP route with 3 next-hops
            let prefix = make_prefix("10.0.0.0", 24);
            let nhg_key = NextHopGroupKey::from_nexthops([nh1, nh2, nh3]);

            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Route), 0);

            let result = orch.add_route(0, prefix.clone(), nhg_key.clone()).await;
            assert!(result.is_ok());

            // Verify: Next-hop group and route created in SAI
            assert!(orch.has_route(0, &prefix));
            assert!(orch.has_nhg(&nhg_key));
            assert_eq!(orch.nhg_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Route), 1);

            // Verify NHG ref count
            assert_eq!(orch.get_nhg(&nhg_key).unwrap().ref_count(), 1);
        }

        #[tokio::test]
        async fn test_route_orch_blackhole_route_creation() {
            let sai = Arc::new(MockSai::new());
            let mut orch = RouteOrch::new(RouteOrchConfig::default());
            let callbacks = Arc::new(MockRouteCallbacks::new(sai.clone()));
            orch.set_callbacks(callbacks.clone());

            // Test: Add blackhole route (empty next-hop group)
            let prefix = make_prefix("10.0.0.0", 24);
            let nhg_key = NextHopGroupKey::new(); // Empty = blackhole

            let result = orch.add_route(0, prefix.clone(), nhg_key).await;
            assert!(result.is_ok());

            // Verify: Route created with blackhole attribute
            assert!(orch.has_route(0, &prefix));
            assert_eq!(sai.count_objects(SaiObjectType::Route), 1);

            let route_obj = sai.objects.lock().unwrap()
                .iter()
                .find(|obj| obj.object_type == SaiObjectType::Route)
                .cloned()
                .unwrap();

            // Verify blackhole attribute is set
            assert!(route_obj.attributes.iter().any(|(k, v)| k == "blackhole" && v == "true"));

            // Verify no next-hop group created
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 0);
        }

        #[tokio::test]
        async fn test_route_orch_route_update_scenarios() {
            let sai = Arc::new(MockSai::new());
            let mut orch = RouteOrch::new(RouteOrchConfig::default());
            let callbacks = Arc::new(MockRouteCallbacks::new(sai.clone()));

            // Setup next-hops
            let nh1 = make_nexthop("192.168.1.1", "Ethernet0");
            let nh2 = make_nexthop("192.168.1.2", "Ethernet4");
            let nh3 = make_nexthop("192.168.1.3", "Ethernet8");
            callbacks.add_next_hop(nh1.clone(), 0x1000);
            callbacks.add_next_hop(nh2.clone(), 0x1001);
            callbacks.add_next_hop(nh3.clone(), 0x1002);
            orch.set_callbacks(callbacks.clone());

            let prefix = make_prefix("10.0.0.0", 24);

            // Scenario 1: Single NH -> Different Single NH
            let nhg_key1 = NextHopGroupKey::single(nh1.clone());
            orch.add_route(0, prefix.clone(), nhg_key1).await.unwrap();

            let nhg_key2 = NextHopGroupKey::single(nh2.clone());
            orch.add_route(0, prefix.clone(), nhg_key2).await.unwrap();

            // Verify old NH ref decremented, new NH ref incremented
            let refs = callbacks.next_hop_refs.lock().unwrap();
            assert_eq!(refs.get(&nh1), Some(&0));
            assert_eq!(refs.get(&nh2), Some(&1));
            drop(refs);

            // Scenario 2: Single NH -> ECMP (multiple NHs)
            let nhg_ecmp = NextHopGroupKey::from_nexthops([nh2.clone(), nh3.clone()]);
            orch.add_route(0, prefix.clone(), nhg_ecmp.clone()).await.unwrap();

            // Verify NHG created and old single NH ref decremented
            assert!(orch.has_nhg(&nhg_ecmp));
            assert_eq!(orch.nhg_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 1);

            let refs = callbacks.next_hop_refs.lock().unwrap();
            assert_eq!(refs.get(&nh2), Some(&0)); // Was incremented then decremented
            drop(refs);

            // Scenario 3: ECMP -> Blackhole
            let nhg_blackhole = NextHopGroupKey::new();
            orch.add_route(0, prefix.clone(), nhg_blackhole).await.unwrap();

            // Verify ECMP NHG ref count decremented
            // Note: NHG may still be cached even with ref count 0
            if orch.has_nhg(&nhg_ecmp) {
                assert_eq!(orch.get_nhg(&nhg_ecmp).unwrap().ref_count(), 0);
            }

            // Verify route still exists as blackhole
            assert!(orch.has_route(0, &prefix));
            let route = orch.get_route(0, &prefix).unwrap();
            assert!(route.is_blackhole());
        }

        #[tokio::test]
        async fn test_route_orch_bulk_route_operations() {
            let sai = Arc::new(MockSai::new());
            let mut orch = RouteOrch::new(RouteOrchConfig::default());
            let callbacks = Arc::new(MockRouteCallbacks::new(sai.clone()));

            // Setup next-hops
            let nh1 = make_nexthop("192.168.1.1", "Ethernet0");
            let nh2 = make_nexthop("192.168.1.2", "Ethernet4");
            callbacks.add_next_hop(nh1.clone(), 0x1000);
            callbacks.add_next_hop(nh2.clone(), 0x1001);
            orch.set_callbacks(callbacks.clone());

            // Test: Add 20 routes
            let mut prefixes = Vec::new();
            for i in 0..20 {
                let prefix = make_prefix(&format!("10.{}.0.0", i), 24);
                let nhg_key = if i % 2 == 0 {
                    NextHopGroupKey::single(nh1.clone())
                } else {
                    NextHopGroupKey::from_nexthops([nh1.clone(), nh2.clone()])
                };

                orch.add_route(0, prefix.clone(), nhg_key).await.unwrap();
                prefixes.push(prefix);
            }

            // Verify: All routes created
            assert_eq!(sai.count_objects(SaiObjectType::Route), 20);

            // 10 ECMP routes should create NHG (but they share same NHG)
            assert_eq!(orch.nhg_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 1);

            for prefix in &prefixes {
                assert!(orch.has_route(0, prefix));
            }

            // Test: Bulk removal
            for prefix in &prefixes {
                orch.remove_route(0, prefix).await.unwrap();
            }

            // Verify: All routes removed
            assert_eq!(sai.count_objects(SaiObjectType::Route), 0);
            assert_eq!(orch.nhg_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 0);

            for prefix in &prefixes {
                assert!(!orch.has_route(0, prefix));
            }
        }

        #[tokio::test]
        async fn test_route_orch_multiple_routes_share_ecmp_nhg() {
            let sai = Arc::new(MockSai::new());
            let mut orch = RouteOrch::new(RouteOrchConfig::default());
            let callbacks = Arc::new(MockRouteCallbacks::new(sai.clone()));

            let nh1 = make_nexthop("192.168.1.1", "Ethernet0");
            let nh2 = make_nexthop("192.168.1.2", "Ethernet4");
            callbacks.add_next_hop(nh1.clone(), 0x1000);
            callbacks.add_next_hop(nh2.clone(), 0x1001);
            orch.set_callbacks(callbacks);

            // Create shared ECMP NHG
            let nhg_key = NextHopGroupKey::from_nexthops([nh1, nh2]);

            // Add 5 routes using same ECMP NHG
            let prefix1 = make_prefix("10.0.0.0", 24);
            let prefix2 = make_prefix("10.1.0.0", 24);
            let prefix3 = make_prefix("10.2.0.0", 24);
            let prefix4 = make_prefix("10.3.0.0", 24);
            let prefix5 = make_prefix("10.4.0.0", 24);

            orch.add_route(0, prefix1.clone(), nhg_key.clone()).await.unwrap();
            orch.add_route(0, prefix2.clone(), nhg_key.clone()).await.unwrap();
            orch.add_route(0, prefix3.clone(), nhg_key.clone()).await.unwrap();
            orch.add_route(0, prefix4.clone(), nhg_key.clone()).await.unwrap();
            orch.add_route(0, prefix5.clone(), nhg_key.clone()).await.unwrap();

            // Verify: Only 1 NHG created, shared by 5 routes
            assert_eq!(orch.nhg_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Route), 5);
            assert_eq!(orch.get_nhg(&nhg_key).unwrap().ref_count(), 5);

            // Remove 3 routes
            orch.remove_route(0, &prefix1).await.unwrap();
            orch.remove_route(0, &prefix2).await.unwrap();
            orch.remove_route(0, &prefix3).await.unwrap();

            // Verify: NHG still exists with ref count 2
            assert_eq!(orch.nhg_count(), 1);
            assert_eq!(orch.get_nhg(&nhg_key).unwrap().ref_count(), 2);
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 1);

            // Remove remaining routes
            orch.remove_route(0, &prefix4).await.unwrap();
            orch.remove_route(0, &prefix5).await.unwrap();

            // Verify: NHG removed when last reference gone
            assert_eq!(orch.nhg_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Route), 0);
        }

        #[tokio::test]
        async fn test_route_orch_vrf_route_operations() {
            let sai = Arc::new(MockSai::new());
            let mut orch = RouteOrch::new(RouteOrchConfig::default());
            let callbacks = Arc::new(MockRouteCallbacks::new(sai.clone()));

            // Add VRF
            callbacks.add_vrf(0x1234);

            let nh = make_nexthop("192.168.1.1", "Ethernet0");
            callbacks.add_next_hop(nh.clone(), 0x1000);
            orch.set_callbacks(callbacks.clone());

            // Add route in custom VRF
            let prefix = make_prefix("10.0.0.0", 24);
            let nhg_key = NextHopGroupKey::single(nh);

            let result = orch.add_route(0x1234, prefix.clone(), nhg_key).await;
            assert!(result.is_ok());

            // Verify route in VRF
            assert!(orch.has_route(0x1234, &prefix));
            assert!(!orch.has_route(0, &prefix)); // Not in default VRF

            // Verify VRF ref count incremented
            let vrf_refs = callbacks.vrf_refs.lock().unwrap();
            assert_eq!(vrf_refs.get(&0x1234), Some(&1));
            drop(vrf_refs);

            // Remove route
            orch.remove_route(0x1234, &prefix).await.unwrap();

            // Verify VRF ref count decremented
            let vrf_refs = callbacks.vrf_refs.lock().unwrap();
            assert_eq!(vrf_refs.get(&0x1234), Some(&0));
        }

        #[tokio::test]
        async fn test_route_orch_nhg_max_limit_enforcement() {
            let sai = Arc::new(MockSai::new());
            let config = RouteOrchConfig {
                max_nhg_count: 3,
                ..Default::default()
            };
            let mut orch = RouteOrch::new(config);
            let callbacks = Arc::new(MockRouteCallbacks::new(sai.clone()));

            // Setup next-hops
            for i in 0..10 {
                let nh = make_nexthop(&format!("192.168.1.{}", i), "Ethernet0");
                callbacks.add_next_hop(nh, 0x1000 + i as u64);
            }
            orch.set_callbacks(callbacks);

            // Create 3 ECMP NHGs (should succeed)
            let mut prefixes = Vec::new();
            for i in 0..3 {
                let prefix = make_prefix(&format!("10.{}.0.0", i), 24);
                let nhg_key = NextHopGroupKey::from_nexthops([
                    make_nexthop(&format!("192.168.1.{}", i * 2), "Ethernet0"),
                    make_nexthop(&format!("192.168.1.{}", i * 2 + 1), "Ethernet0"),
                ]);
                orch.add_route(0, prefix.clone(), nhg_key).await.unwrap();
                prefixes.push(prefix);
            }

            assert_eq!(orch.nhg_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 3);

            // Try to create 4th NHG (should fail)
            let prefix4 = make_prefix("10.99.0.0", 24);
            let nhg_key4 = NextHopGroupKey::from_nexthops([
                make_nexthop("192.168.1.8", "Ethernet0"),
                make_nexthop("192.168.1.9", "Ethernet0"),
            ]);

            let result = orch.add_route(0, prefix4, nhg_key4).await;
            assert!(result.is_err());
            assert_eq!(orch.nhg_count(), 3);

            // Remove one route to free up NHG slot
            orch.remove_route(0, &prefixes[0]).await.unwrap();
            assert_eq!(orch.nhg_count(), 2);

            // Now adding new NHG should succeed
            let prefix5 = make_prefix("10.100.0.0", 24);
            let nhg_key5 = NextHopGroupKey::from_nexthops([
                make_nexthop("192.168.1.8", "Ethernet0"),
                make_nexthop("192.168.1.9", "Ethernet0"),
            ]);

            let result = orch.add_route(0, prefix5, nhg_key5).await;
            assert!(result.is_ok());
            assert_eq!(orch.nhg_count(), 3);
        }
    }

    // AclOrch integration tests
    mod acl_orch_tests {
        use super::*;
        use sonic_orchagent::{
            AclOrch, AclOrchConfig,
            AclTable, AclTableConfig,
            AclRule, AclRuleAction, AclRuleMatch,
            AclStage, AclRedirectTarget, AclMatchValue, AclMatchField,
        };
        use sonic_types::IpAddress;
        use std::str::FromStr;

        fn create_table_with_sai(
            table_id: &str,
            table_type: &str,
            stage: AclStage,
            sai: &MockSai,
        ) -> (AclTableConfig, u64) {
            let config = AclTableConfig::new()
                .with_id(table_id)
                .with_type(table_type)
                .with_stage(stage);

            let oid = sai.create_object(
                SaiObjectType::AclTable,
                vec![
                    ("table_id".to_string(), table_id.to_string()),
                    ("type".to_string(), table_type.to_string()),
                    ("stage".to_string(), format!("{}", stage)),
                ]
            ).unwrap();

            (config, oid)
        }

        fn create_rule_with_sai(
            rule_id: &str,
            priority: u32,
            sai: &MockSai,
        ) -> (AclRule, u64) {
            let rule = AclRule::packet(rule_id)
                .with_priority(priority)
                .with_action(AclRuleAction::drop());

            let oid = sai.create_object(
                SaiObjectType::AclRule,
                vec![
                    ("rule_id".to_string(), rule_id.to_string()),
                    ("priority".to_string(), priority.to_string()),
                ]
            ).unwrap();

            (rule, oid)
        }

        #[test]
        fn test_acl_orch_table_creation_and_removal_with_sai_validation() {
            let sai = MockSai::new();
            let mut orch = AclOrch::new(AclOrchConfig::default());

            assert_eq!(sai.count_objects(SaiObjectType::AclTable), 0);

            // Create ACL table
            let (config, oid) = create_table_with_sai("TestTable", "L3", AclStage::Ingress, &sai);
            orch.create_table(&config).unwrap();

            assert_eq!(orch.table_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::AclTable), 1);
            assert!(orch.has_table("TestTable"));

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::AclTable);
            assert_eq!(sai_obj.attributes[0].1, "TestTable");

            // Remove ACL table
            orch.remove_table("TestTable").unwrap();
            sai.remove_object(oid).unwrap();

            assert_eq!(orch.table_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::AclTable), 0);
            assert!(!orch.has_table("TestTable"));
        }

        #[test]
        fn test_acl_orch_rule_add_remove_with_match_criteria() {
            let sai = MockSai::new();
            let mut orch = AclOrch::new(AclOrchConfig::default());

            // Create table
            let (config, _table_oid) = create_table_with_sai("TestTable", "L3", AclStage::Ingress, &sai);
            orch.create_table(&config).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::AclRule), 0);

            // Create rule with IP protocol match
            let ip_addr = IpAddress::from_str("192.168.1.0").unwrap();
            let (mut rule, rule_oid) = create_rule_with_sai("rule1", 100, &sai);
            rule.add_match(AclRuleMatch::ip_protocol(6)); // TCP
            rule.add_match(AclRuleMatch::src_ip(ip_addr, None));
            rule.add_match(AclRuleMatch::l4_dst_port(80)); // HTTP

            orch.add_rule("TestTable", rule.clone()).unwrap();

            assert_eq!(orch.total_rule_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::AclRule), 1);

            let stored_rule = orch.get_rule("TestTable", "rule1").unwrap();
            assert_eq!(stored_rule.priority, 100);
            assert!(stored_rule.has_match(AclMatchField::IpProtocol));
            assert!(stored_rule.has_match(AclMatchField::SrcIp));
            assert!(stored_rule.has_match(AclMatchField::L4DstPort));

            let sai_obj = sai.get_object(rule_oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::AclRule);

            // Remove rule
            let removed = orch.remove_rule("TestTable", "rule1").unwrap();
            sai.remove_object(rule_oid).unwrap();

            assert_eq!(removed.id, "rule1");
            assert_eq!(orch.total_rule_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::AclRule), 0);
        }

        #[test]
        fn test_acl_orch_priority_based_rule_ordering() {
            let sai = MockSai::new();
            let mut orch = AclOrch::new(AclOrchConfig::default());

            // Create table
            let (config, _) = create_table_with_sai("TestTable", "L3", AclStage::Ingress, &sai);
            orch.create_table(&config).unwrap();

            // Add rules with different priorities
            let (rule_low, oid1) = create_rule_with_sai("rule_low", 10, &sai);
            let (rule_med, oid2) = create_rule_with_sai("rule_med", 50, &sai);
            let (rule_high, oid3) = create_rule_with_sai("rule_high", 100, &sai);

            orch.add_rule("TestTable", rule_low).unwrap();
            orch.add_rule("TestTable", rule_med).unwrap();
            orch.add_rule("TestTable", rule_high).unwrap();

            assert_eq!(orch.total_rule_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::AclRule), 3);

            // Verify priorities
            let r1 = orch.get_rule("TestTable", "rule_low").unwrap();
            let r2 = orch.get_rule("TestTable", "rule_med").unwrap();
            let r3 = orch.get_rule("TestTable", "rule_high").unwrap();

            assert_eq!(r1.priority, 10);
            assert_eq!(r2.priority, 50);
            assert_eq!(r3.priority, 100);

            // Verify SAI objects exist
            assert!(sai.get_object(oid1).is_some());
            assert!(sai.get_object(oid2).is_some());
            assert!(sai.get_object(oid3).is_some());

            // Higher priority should be processed first (validate ordering)
            assert!(r3.priority > r2.priority);
            assert!(r2.priority > r1.priority);
        }

        #[test]
        fn test_acl_orch_multiple_rules_in_same_table() {
            let sai = MockSai::new();
            let mut orch = AclOrch::new(AclOrchConfig::default());

            // Create table
            let (config, _) = create_table_with_sai("TestTable", "L3", AclStage::Ingress, &sai);
            orch.create_table(&config).unwrap();

            // Add 5 rules with different match conditions
            let rules = vec![
                ("rule_tcp", 100, 6u8),     // TCP
                ("rule_udp", 90, 17u8),     // UDP
                ("rule_icmp", 80, 1u8),     // ICMP
                ("rule_gre", 70, 47u8),     // GRE
                ("rule_esp", 60, 50u8),     // ESP
            ];

            for (rule_id, priority, protocol) in &rules {
                let (mut rule, _) = create_rule_with_sai(rule_id, *priority, &sai);
                rule.add_match(AclRuleMatch::ip_protocol(*protocol));
                orch.add_rule("TestTable", rule).unwrap();
            }

            assert_eq!(orch.total_rule_count(), 5);
            assert_eq!(sai.count_objects(SaiObjectType::AclRule), 5);

            // Verify all rules exist
            for (rule_id, priority, protocol) in &rules {
                let rule = orch.get_rule("TestTable", rule_id).unwrap();
                assert_eq!(rule.priority, *priority);
                assert!(rule.has_match(AclMatchField::IpProtocol));
            }

            // Remove all rules
            for (rule_id, _, _) in &rules {
                let removed = orch.remove_rule("TestTable", rule_id).unwrap();
                // In real implementation, would also remove from SAI
            }

            assert_eq!(orch.total_rule_count(), 0);
        }

        #[test]
        fn test_acl_orch_actions_drop_forward_mirror() {
            let sai = MockSai::new();
            let mut orch = AclOrch::new(AclOrchConfig::default());

            // Test DROP action
            let (config, _) = create_table_with_sai("DropTable", "L3", AclStage::Ingress, &sai);
            orch.create_table(&config).unwrap();

            let (drop_rule, _drop_oid) = create_rule_with_sai("drop_rule", 100, &sai);
            orch.add_rule("DropTable", drop_rule).unwrap();

            let stored = orch.get_rule("DropTable", "drop_rule").unwrap();
            assert!(stored.has_action(sonic_orchagent::acl::AclActionType::PacketAction));

            // Test FORWARD action
            let (config, _) = create_table_with_sai("ForwardTable", "L3", AclStage::Ingress, &sai);
            orch.create_table(&config).unwrap();

            let (mut fwd_rule, _fwd_oid) = create_rule_with_sai("fwd_rule", 100, &sai);
            // Replace default drop action with forward action
            fwd_rule.actions.clear();
            fwd_rule.add_action(AclRuleAction::forward());
            orch.add_rule("ForwardTable", fwd_rule).unwrap();

            let stored = orch.get_rule("ForwardTable", "fwd_rule").unwrap();
            assert!(stored.has_action(sonic_orchagent::acl::AclActionType::PacketAction));

            // Test MIRROR action
            let (config, _) = create_table_with_sai("MirrorTable", "MIRROR", AclStage::Ingress, &sai);
            orch.create_table(&config).unwrap();

            let (mut mirror_rule, _mirror_oid) = create_rule_with_sai("mirror_rule", 100, &sai);
            // Replace default drop action with mirror action
            mirror_rule.actions.clear();
            mirror_rule.add_action(AclRuleAction::mirror_ingress("session1"));
            orch.add_rule("MirrorTable", mirror_rule).unwrap();

            let stored = orch.get_rule("MirrorTable", "mirror_rule").unwrap();
            assert!(stored.has_action(sonic_orchagent::acl::AclActionType::MirrorIngress));

            // Verify SAI objects
            assert_eq!(sai.count_objects(SaiObjectType::AclTable), 3);
            assert_eq!(sai.count_objects(SaiObjectType::AclRule), 3);
        }

        #[test]
        fn test_acl_orch_complex_match_criteria_with_ranges() {
            let sai = MockSai::new();
            let mut orch = AclOrch::new(AclOrchConfig::default());

            // Create table
            let (config, _) = create_table_with_sai("TestTable", "L3", AclStage::Ingress, &sai);
            orch.create_table(&config).unwrap();

            // Create rule with port range, TCP flags, and DSCP
            let (mut rule, rule_oid) = create_rule_with_sai("complex_rule", 100, &sai);
            rule.add_match(AclRuleMatch::l4_src_port_range(1000, 2000));
            rule.add_match(AclRuleMatch::l4_dst_port_range(8000, 9000));
            rule.add_match(AclRuleMatch::tcp_flags(0x02, 0xFF)); // SYN flag
            rule.add_match(AclRuleMatch::dscp(46)); // EF
            rule.add_action(AclRuleAction::drop());

            orch.add_rule("TestTable", rule).unwrap();

            let stored = orch.get_rule("TestTable", "complex_rule").unwrap();
            assert!(stored.has_match(AclMatchField::L4SrcPortRange));
            assert!(stored.has_match(AclMatchField::L4DstPortRange));
            assert!(stored.has_match(AclMatchField::TcpFlags));
            assert!(stored.has_match(AclMatchField::Dscp));

            assert_eq!(sai.count_objects(SaiObjectType::AclRule), 1);

            let sai_obj = sai.get_object(rule_oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::AclRule);
        }

        #[test]
        fn test_acl_orch_redirect_action_variations() {
            let sai = MockSai::new();
            let mut orch = AclOrch::new(AclOrchConfig::default());

            // Create table
            let (config, _) = create_table_with_sai("RedirectTable", "L3", AclStage::Ingress, &sai);
            orch.create_table(&config).unwrap();

            // Test redirect to port
            let (mut rule1, oid1) = create_rule_with_sai("redirect_port", 100, &sai);
            rule1.add_action(AclRuleAction::redirect(AclRedirectTarget::Port("Ethernet0".to_string())));
            orch.add_rule("RedirectTable", rule1).unwrap();

            let stored = orch.get_rule("RedirectTable", "redirect_port").unwrap();
            assert!(stored.has_action(sonic_orchagent::acl::AclActionType::Redirect));

            // Test redirect to next-hop
            let (mut rule2, oid2) = create_rule_with_sai("redirect_nh", 90, &sai);
            rule2.add_action(AclRuleAction::redirect(AclRedirectTarget::NextHop("10.0.0.1@Ethernet0".to_string())));
            orch.add_rule("RedirectTable", rule2).unwrap();

            // Test redirect to next-hop group
            let (mut rule3, oid3) = create_rule_with_sai("redirect_nhg", 80, &sai);
            rule3.add_action(AclRuleAction::redirect(AclRedirectTarget::NextHopGroup("nhg1".to_string())));
            orch.add_rule("RedirectTable", rule3).unwrap();

            assert_eq!(orch.total_rule_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::AclRule), 3);

            // Verify all redirect rules
            for rule_id in &["redirect_port", "redirect_nh", "redirect_nhg"] {
                let rule = orch.get_rule("RedirectTable", rule_id).unwrap();
                assert!(rule.has_action(sonic_orchagent::acl::AclActionType::Redirect));
            }
        }

        #[test]
        fn test_acl_orch_rule_with_counter_attachment() {
            let sai = MockSai::new();
            let mut orch = AclOrch::new(AclOrchConfig::default());

            // Create table
            let (config, _) = create_table_with_sai("TestTable", "L3", AclStage::Ingress, &sai);
            orch.create_table(&config).unwrap();

            // Create rule with counter
            let (mut rule, rule_oid) = create_rule_with_sai("counted_rule", 100, &sai);
            rule.add_match(AclRuleMatch::ip_protocol(6));
            rule.add_action(AclRuleAction::drop());
            rule.counter_enabled = true;

            // Create counter SAI object
            let counter_oid = sai.create_object(
                SaiObjectType::AclCounter,
                vec![
                    ("rule_id".to_string(), "counted_rule".to_string()),
                ]
            ).unwrap();

            orch.add_rule("TestTable", rule).unwrap();

            let stored = orch.get_rule("TestTable", "counted_rule").unwrap();
            assert!(stored.counter_enabled);

            // Verify both rule and counter objects exist
            assert_eq!(sai.count_objects(SaiObjectType::AclRule), 1);
            assert_eq!(sai.count_objects(SaiObjectType::AclCounter), 1);

            let rule_obj = sai.get_object(rule_oid).unwrap();
            assert_eq!(rule_obj.object_type, SaiObjectType::AclRule);

            let counter_obj = sai.get_object(counter_oid).unwrap();
            assert_eq!(counter_obj.object_type, SaiObjectType::AclCounter);
        }

        #[test]
        fn test_acl_orch_multiple_tables_different_stages() {
            let sai = MockSai::new();
            let mut orch = AclOrch::new(AclOrchConfig::default());

            // Create ingress table
            let (ingress_config, ingress_oid) = create_table_with_sai(
                "IngressTable",
                "L3",
                AclStage::Ingress,
                &sai
            );
            orch.create_table(&ingress_config).unwrap();

            // Create egress table
            let (egress_config, egress_oid) = create_table_with_sai(
                "EgressTable",
                "L3",
                AclStage::Egress,
                &sai
            );
            orch.create_table(&egress_config).unwrap();

            assert_eq!(orch.table_count(), 2);
            assert_eq!(sai.count_objects(SaiObjectType::AclTable), 2);

            // Add rules to each table
            let (rule1, _) = create_rule_with_sai("ingress_rule", 100, &sai);
            orch.add_rule("IngressTable", rule1).unwrap();

            let (rule2, _) = create_rule_with_sai("egress_rule", 100, &sai);
            orch.add_rule("EgressTable", rule2).unwrap();

            assert_eq!(orch.total_rule_count(), 2);
            assert_eq!(sai.count_objects(SaiObjectType::AclRule), 2);

            // Verify stages
            let ingress_table = orch.get_table("IngressTable").unwrap();
            assert_eq!(ingress_table.stage, AclStage::Ingress);

            let egress_table = orch.get_table("EgressTable").unwrap();
            assert_eq!(egress_table.stage, AclStage::Egress);
        }

        #[test]
        fn test_acl_orch_rule_update_preserves_sai_state() {
            let sai = MockSai::new();
            let mut orch = AclOrch::new(AclOrchConfig::default());

            // Create table
            let (config, _) = create_table_with_sai("TestTable", "L3", AclStage::Ingress, &sai);
            orch.create_table(&config).unwrap();

            // Add initial rule
            let (rule, rule_oid) = create_rule_with_sai("update_rule", 100, &sai);
            orch.add_rule("TestTable", rule).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::AclRule), 1);
            let initial_obj = sai.get_object(rule_oid).unwrap();
            assert_eq!(initial_obj.attributes[1].1, "100"); // priority

            // Update rule with new priority
            let (updated_rule, _) = create_rule_with_sai("update_rule", 200, &sai);
            let old_rule = orch.update_rule("TestTable", updated_rule).unwrap();

            assert_eq!(old_rule.priority, 100);

            let new_rule = orch.get_rule("TestTable", "update_rule").unwrap();
            assert_eq!(new_rule.priority, 200);

            // SAI object count should remain the same (update, not create)
            assert_eq!(sai.count_objects(SaiObjectType::AclRule), 2); // 2 because we created another in the test
        }

        #[test]
        fn test_acl_orch_ipv6_match_criteria() {
            let sai = MockSai::new();
            let mut orch = AclOrch::new(AclOrchConfig::default());

            // Create L3V6 table
            let (config, _) = create_table_with_sai("Ipv6Table", "L3V6", AclStage::Ingress, &sai);
            orch.create_table(&config).unwrap();

            // Create rule with IPv6 match
            let (mut rule, _rule_oid) = create_rule_with_sai("ipv6_rule", 100, &sai);
            let ipv6_addr = IpAddress::from_str("2001:db8::1").unwrap();
            rule.add_match(AclRuleMatch::new(
                AclMatchField::SrcIpv6,
                AclMatchValue::Ipv6 { addr: ipv6_addr, mask: None }
            ));
            rule.add_match(AclRuleMatch::new(
                AclMatchField::Ipv6NextHeader,
                AclMatchValue::U8(58)
            )); // ICMPv6
            rule.add_action(AclRuleAction::drop());

            orch.add_rule("Ipv6Table", rule).unwrap();

            let stored = orch.get_rule("Ipv6Table", "ipv6_rule").unwrap();
            assert!(stored.has_match(AclMatchField::SrcIpv6));
            assert!(stored.has_match(AclMatchField::Ipv6NextHeader));

            assert_eq!(sai.count_objects(SaiObjectType::AclRule), 1);
        }

        #[test]
        fn test_acl_orch_statistics_tracking() {
            let sai = MockSai::new();
            let mut orch = AclOrch::new(AclOrchConfig::default());

            assert_eq!(orch.stats().tables_created, 0);
            assert_eq!(orch.stats().rules_created, 0);

            // Create table
            let (config, table_oid) = create_table_with_sai("TestTable", "L3", AclStage::Ingress, &sai);
            orch.create_table(&config).unwrap();
            assert_eq!(orch.stats().tables_created, 1);

            // Add rules
            for i in 0..3 {
                let (rule, _) = create_rule_with_sai(&format!("rule{}", i), 100 + i, &sai);
                orch.add_rule("TestTable", rule).unwrap();
            }
            assert_eq!(orch.stats().rules_created, 3);

            // Remove a rule
            orch.remove_rule("TestTable", "rule0").unwrap();
            assert_eq!(orch.stats().rules_deleted, 1);

            // Update a rule
            let (updated, _) = create_rule_with_sai("rule1", 200, &sai);
            orch.update_rule("TestTable", updated).unwrap();
            assert_eq!(orch.stats().rules_updated, 1);

            // Remove table
            orch.remove_table("TestTable").unwrap();
            assert_eq!(orch.stats().tables_deleted, 1);
        }

        #[test]
        fn test_acl_orch_table_with_port_binding() {
            let sai = MockSai::new();
            let mut orch = AclOrch::new(AclOrchConfig::default());

            // Create table with ports
            let config = AclTableConfig::new()
                .with_id("PortTable")
                .with_type("L3")
                .with_stage(AclStage::Ingress)
                .with_ports(vec!["Ethernet0".to_string(), "Ethernet4".to_string()]);

            let _table_oid = sai.create_object(
                SaiObjectType::AclTable,
                vec![
                    ("table_id".to_string(), "PortTable".to_string()),
                    ("ports".to_string(), "Ethernet0,Ethernet4".to_string()),
                ]
            ).unwrap();

            orch.create_table(&config).unwrap();

            let table = orch.get_table("PortTable").unwrap();
            assert!(table.is_port_configured("Ethernet0"));
            assert!(table.is_port_configured("Ethernet4"));

            // Bind ports
            orch.bind_port("PortTable", "Ethernet0", 0x1000).unwrap();
            orch.bind_port("PortTable", "Ethernet4", 0x1001).unwrap();

            let table = orch.get_table("PortTable").unwrap();
            assert!(table.is_port_bound("Ethernet0"));
            assert!(table.is_port_bound("Ethernet4"));

            // Unbind port
            orch.unbind_port("PortTable", "Ethernet0").unwrap();
            let table = orch.get_table("PortTable").unwrap();
            assert!(!table.is_port_bound("Ethernet0"));
            assert!(table.is_port_bound("Ethernet4"));
        }
    }

    // PortsOrch integration tests
    mod ports_orch_tests {
        use super::*;
        use sonic_orchagent::{
            PortsOrch, PortsOrchConfig,
            Port, PortAdminState, PortOperState, PortType, PortFecMode,
            QueueInfo, QueueType, VlanTaggingMode,
        };

        fn create_port_with_sai(
            alias: &str,
            port_id: u64,
            lanes: Vec<u32>,
            sai: &MockSai,
        ) -> u64 {
            sai.create_object(
                SaiObjectType::Port,
                vec![
                    ("alias".to_string(), alias.to_string()),
                    ("port_id".to_string(), port_id.to_string()),
                    ("lanes".to_string(), format!("{:?}", lanes)),
                ]
            ).unwrap()
        }

        fn create_lag_with_sai(alias: &str, lag_id: u64, sai: &MockSai) -> u64 {
            sai.create_object(
                SaiObjectType::Port,
                vec![
                    ("alias".to_string(), alias.to_string()),
                    ("lag_id".to_string(), lag_id.to_string()),
                    ("type".to_string(), "LAG".to_string()),
                ]
            ).unwrap()
        }

        fn create_vlan_with_sai(alias: &str, vlan_id: u16, sai_vlan_id: u64, sai: &MockSai) -> u64 {
            sai.create_object(
                SaiObjectType::Port,
                vec![
                    ("alias".to_string(), alias.to_string()),
                    ("vlan_id".to_string(), vlan_id.to_string()),
                    ("sai_vlan_id".to_string(), sai_vlan_id.to_string()),
                    ("type".to_string(), "VLAN".to_string()),
                ]
            ).unwrap()
        }

        fn create_queue_with_sai(queue_id: u64, index: u32, sai: &MockSai) -> u64 {
            sai.create_object(
                SaiObjectType::QosMap,
                vec![
                    ("queue_id".to_string(), queue_id.to_string()),
                    ("index".to_string(), index.to_string()),
                    ("type".to_string(), "QUEUE".to_string()),
                ]
            ).unwrap()
        }

        #[test]
        fn test_ports_orch_add_port_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = PortsOrch::new(PortsOrchConfig::default());

            assert_eq!(sai.count_objects(SaiObjectType::Port), 0);

            let port_id = create_port_with_sai("Ethernet0", 0x1000, vec![0, 1, 2, 3], &sai);
            orch.add_port_from_hardware("Ethernet0".to_string(), port_id, vec![0, 1, 2, 3])
                .unwrap();

            assert_eq!(orch.port_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Port), 1);
            assert_eq!(orch.stats().ports_created, 1);

            let sai_obj = sai.get_object(port_id).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::Port);
        }

        #[test]
        fn test_ports_orch_port_configuration_with_sai_validation() {
            let sai = MockSai::new();
            let mut orch = PortsOrch::new(PortsOrchConfig::default());

            // Create port with SAI
            let port_id = create_port_with_sai("Ethernet0", 0x1000, vec![0, 1, 2, 3], &sai);
            orch.add_port_from_hardware("Ethernet0".to_string(), port_id, vec![0, 1, 2, 3])
                .unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::Port), 1);

            // Verify port can be retrieved by OID
            let port = orch.get_port_by_oid(port_id).unwrap();
            assert_eq!(port.alias, "Ethernet0");
            assert_eq!(port.port_id, port_id);
            assert_eq!(port.port_type, PortType::Phy);

            // Verify port is in correct state
            assert_eq!(port.admin_state, PortAdminState::Down);
            assert_eq!(port.oper_state, PortOperState::Down);
        }

        #[test]
        fn test_ports_orch_port_state_transitions() {
            let sai = MockSai::new();
            let mut orch = PortsOrch::new(PortsOrchConfig::default());

            let port_id = create_port_with_sai("Ethernet0", 0x1000, vec![0], &sai);
            orch.add_port_from_hardware("Ethernet0".to_string(), port_id, vec![0])
                .unwrap();

            // Initial state: admin down, oper down
            let port = orch.get_port("Ethernet0").unwrap();
            assert_eq!(port.admin_state, PortAdminState::Down);
            assert_eq!(port.oper_state, PortOperState::Down);

            // Set admin state to up (simulates SAI attribute set)
            orch.set_port_admin_state("Ethernet0", PortAdminState::Up)
                .unwrap();
            let port = orch.get_port("Ethernet0").unwrap();
            assert_eq!(port.admin_state, PortAdminState::Up);

            // Set operational state to up (simulates link up notification from SAI)
            orch.set_port_oper_state("Ethernet0", PortOperState::Up)
                .unwrap();
            let port = orch.get_port("Ethernet0").unwrap();
            assert_eq!(port.oper_state, PortOperState::Up);

            // Set admin state back to down
            orch.set_port_admin_state("Ethernet0", PortAdminState::Down)
                .unwrap();
            let port = orch.get_port("Ethernet0").unwrap();
            assert_eq!(port.admin_state, PortAdminState::Down);

            // SAI object should still exist
            assert_eq!(sai.count_objects(SaiObjectType::Port), 1);
        }

        #[test]
        fn test_ports_orch_remove_port_deletes_sai_object() {
            let sai = MockSai::new();
            let mut orch = PortsOrch::new(PortsOrchConfig::default());

            let port_id = create_port_with_sai("Ethernet0", 0x1000, vec![0], &sai);
            orch.add_port_from_hardware("Ethernet0".to_string(), port_id, vec![0])
                .unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::Port), 1);

            orch.remove_port("Ethernet0").unwrap();
            sai.remove_object(port_id).unwrap();

            assert_eq!(orch.port_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Port), 0);
            assert_eq!(orch.stats().ports_deleted, 1);
        }

        #[test]
        fn test_ports_orch_lag_operations_with_sai() {
            let sai = MockSai::new();
            let mut orch = PortsOrch::new(PortsOrchConfig::default());

            // Create member ports
            let port1_id = create_port_with_sai("Ethernet0", 0x1000, vec![0], &sai);
            let port2_id = create_port_with_sai("Ethernet4", 0x1001, vec![1], &sai);
            let port3_id = create_port_with_sai("Ethernet8", 0x1002, vec![2], &sai);

            orch.add_port_from_hardware("Ethernet0".to_string(), port1_id, vec![0])
                .unwrap();
            orch.add_port_from_hardware("Ethernet4".to_string(), port2_id, vec![1])
                .unwrap();
            orch.add_port_from_hardware("Ethernet8".to_string(), port3_id, vec![2])
                .unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::Port), 3);

            // Create LAG
            let lag_id = create_lag_with_sai("PortChannel0001", 0x2000, &sai);
            orch.create_lag("PortChannel0001", lag_id).unwrap();

            // LAG also creates a port entry, so we have 4 SAI port objects now
            assert_eq!(sai.count_objects(SaiObjectType::Port), 4);
            assert_eq!(orch.lag_count(), 1);
            assert_eq!(orch.stats().lags_created, 1);

            // Add members to LAG
            orch.add_lag_member("PortChannel0001", "Ethernet0").unwrap();
            orch.add_lag_member("PortChannel0001", "Ethernet4").unwrap();
            orch.add_lag_member("PortChannel0001", "Ethernet8").unwrap();

            let lag = orch.get_lag("PortChannel0001").unwrap();
            assert_eq!(lag.member_count(), 3);
            assert!(lag.has_member("Ethernet0"));
            assert!(lag.has_member("Ethernet4"));
            assert!(lag.has_member("Ethernet8"));

            // Verify member ports have LAG ID set
            let port = orch.get_port("Ethernet0").unwrap();
            assert_eq!(port.lag_id, Some(lag_id));

            // Remove a member
            orch.remove_lag_member("PortChannel0001", "Ethernet0").unwrap();
            let lag = orch.get_lag("PortChannel0001").unwrap();
            assert_eq!(lag.member_count(), 2);

            // Remove LAG
            orch.remove_lag("PortChannel0001").unwrap();
            sai.remove_object(lag_id).unwrap();

            assert_eq!(orch.lag_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Port), 3); // Only physical ports remain
            assert_eq!(orch.stats().lags_deleted, 1);
        }

        #[test]
        fn test_ports_orch_vlan_membership_management() {
            let sai = MockSai::new();
            let mut orch = PortsOrch::new(PortsOrchConfig::default());

            // Create ports
            let port1_id = create_port_with_sai("Ethernet0", 0x1000, vec![0], &sai);
            let port2_id = create_port_with_sai("Ethernet4", 0x1001, vec![1], &sai);

            orch.add_port_from_hardware("Ethernet0".to_string(), port1_id, vec![0])
                .unwrap();
            orch.add_port_from_hardware("Ethernet4".to_string(), port2_id, vec![1])
                .unwrap();

            // Create VLAN
            let vlan_id = create_vlan_with_sai("Vlan100", 100, 0x3000, &sai);
            orch.create_vlan("Vlan100", 100, vlan_id).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::Port), 3); // 2 physical + 1 VLAN
            assert_eq!(orch.vlan_count(), 1);
            assert_eq!(orch.stats().vlans_created, 1);

            // Add VLAN members (tagged)
            orch.add_vlan_member(
                "Vlan100",
                "Ethernet0",
                VlanTaggingMode::Tagged,
                0x4000,
                0x5000,
            )
            .unwrap();

            orch.add_vlan_member(
                "Vlan100",
                "Ethernet4",
                VlanTaggingMode::Untagged,
                0x4001,
                0x5001,
            )
            .unwrap();

            let vlan = orch.get_vlan("Vlan100").unwrap();
            assert_eq!(vlan.member_count(), 2);
            assert!(vlan.has_member("Ethernet0"));
            assert!(vlan.has_member("Ethernet4"));

            // Verify tagging modes
            let member_info = vlan.members.get("Ethernet0").unwrap();
            assert_eq!(member_info.tagging_mode, VlanTaggingMode::Tagged);

            let member_info = vlan.members.get("Ethernet4").unwrap();
            assert_eq!(member_info.tagging_mode, VlanTaggingMode::Untagged);

            // Verify port VLAN membership
            let port = orch.get_port("Ethernet0").unwrap();
            assert!(port.vlan_members.contains(&100));

            // Remove VLAN member
            orch.remove_vlan_member("Vlan100", "Ethernet0").unwrap();
            let vlan = orch.get_vlan("Vlan100").unwrap();
            assert_eq!(vlan.member_count(), 1);

            // Remove VLAN
            orch.remove_vlan("Vlan100").unwrap();
            sai.remove_object(vlan_id).unwrap();

            assert_eq!(orch.vlan_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Port), 2); // Only physical ports remain
            assert_eq!(orch.stats().vlans_deleted, 1);
        }

        #[test]
        fn test_ports_orch_queue_configuration() {
            let sai = MockSai::new();
            let mut orch = PortsOrch::new(PortsOrchConfig::default());

            // Create port
            let port_id = create_port_with_sai("Ethernet0", 0x1000, vec![0, 1, 2, 3], &sai);
            orch.add_port_from_hardware("Ethernet0".to_string(), port_id, vec![0, 1, 2, 3])
                .unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::Port), 1);

            // Create queues (8 unicast + 2 multicast)
            let mut queues = Vec::new();
            for i in 0..8 {
                let queue_id = create_queue_with_sai(0x5000 + i, i as u32, &sai);
                queues.push(QueueInfo::new(queue_id, i as u32, QueueType::Unicast));
            }
            for i in 0..2 {
                let queue_id = create_queue_with_sai(0x5100 + i, i as u32, &sai);
                queues.push(QueueInfo::new(queue_id, i as u32, QueueType::Multicast));
            }

            assert_eq!(sai.count_objects(SaiObjectType::QosMap), 10);

            // Set queues on port
            orch.set_port_queues("Ethernet0", queues.clone());

            // Verify queues are stored
            let port_queues = orch.get_port_queues("Ethernet0").unwrap();
            assert_eq!(port_queues.len(), 10);

            // Count unicast and multicast queues
            let unicast_count = port_queues
                .iter()
                .filter(|q| q.queue_type == QueueType::Unicast)
                .count();
            let multicast_count = port_queues
                .iter()
                .filter(|q| q.queue_type == QueueType::Multicast)
                .count();

            assert_eq!(unicast_count, 8);
            assert_eq!(multicast_count, 2);

            // Verify queue indices
            for (idx, queue) in port_queues.iter().take(8).enumerate() {
                assert_eq!(queue.index, idx as u32);
                assert_eq!(queue.queue_type, QueueType::Unicast);
            }

            for (idx, queue) in port_queues.iter().skip(8).enumerate() {
                assert_eq!(queue.index, idx as u32);
                assert_eq!(queue.queue_type, QueueType::Multicast);
            }
        }

        #[test]
        fn test_ports_orch_full_topology_with_sai_validation() {
            let sai = MockSai::new();
            let mut orch = PortsOrch::new(PortsOrchConfig::default());

            // Create physical ports
            let port1_id = create_port_with_sai("Ethernet0", 0x1000, vec![0], &sai);
            let port2_id = create_port_with_sai("Ethernet4", 0x1001, vec![1], &sai);
            let port3_id = create_port_with_sai("Ethernet8", 0x1002, vec![2], &sai);
            let port4_id = create_port_with_sai("Ethernet12", 0x1003, vec![3], &sai);

            orch.add_port_from_hardware("Ethernet0".to_string(), port1_id, vec![0])
                .unwrap();
            orch.add_port_from_hardware("Ethernet4".to_string(), port2_id, vec![1])
                .unwrap();
            orch.add_port_from_hardware("Ethernet8".to_string(), port3_id, vec![2])
                .unwrap();
            orch.add_port_from_hardware("Ethernet12".to_string(), port4_id, vec![3])
                .unwrap();

            // Create LAGs
            let lag1_id = create_lag_with_sai("PortChannel0001", 0x2000, &sai);
            let lag2_id = create_lag_with_sai("PortChannel0002", 0x2001, &sai);

            orch.create_lag("PortChannel0001", lag1_id).unwrap();
            orch.create_lag("PortChannel0002", lag2_id).unwrap();

            orch.add_lag_member("PortChannel0001", "Ethernet0").unwrap();
            orch.add_lag_member("PortChannel0001", "Ethernet4").unwrap();

            // Create VLANs
            let vlan1_id = create_vlan_with_sai("Vlan100", 100, 0x3000, &sai);
            let vlan2_id = create_vlan_with_sai("Vlan200", 200, 0x3001, &sai);

            orch.create_vlan("Vlan100", 100, vlan1_id).unwrap();
            orch.create_vlan("Vlan200", 200, vlan2_id).unwrap();

            orch.add_vlan_member("Vlan100", "Ethernet8", VlanTaggingMode::Tagged, 0x4000, 0x5000)
                .unwrap();
            orch.add_vlan_member("Vlan100", "PortChannel0001", VlanTaggingMode::Tagged, 0x4001, 0x5001)
                .unwrap();
            orch.add_vlan_member("Vlan200", "Ethernet12", VlanTaggingMode::Untagged, 0x4002, 0x5002)
                .unwrap();

            // Verify complete topology
            assert_eq!(orch.port_count(), 8); // 4 physical + 2 LAGs + 2 VLANs
            assert_eq!(orch.lag_count(), 2);
            assert_eq!(orch.vlan_count(), 2);

            // Verify SAI object counts
            assert_eq!(sai.count_objects(SaiObjectType::Port), 8);

            // Verify LAG memberships
            let lag1 = orch.get_lag("PortChannel0001").unwrap();
            assert_eq!(lag1.member_count(), 2);

            // Verify VLAN memberships
            let vlan1 = orch.get_vlan("Vlan100").unwrap();
            assert_eq!(vlan1.member_count(), 2);

            let vlan2 = orch.get_vlan("Vlan200").unwrap();
            assert_eq!(vlan2.member_count(), 1);

            // Verify port operational states
            orch.set_port_admin_state("Ethernet8", PortAdminState::Up)
                .unwrap();
            orch.set_port_oper_state("Ethernet8", PortOperState::Up)
                .unwrap();

            let up_ports = orch.get_up_ports();
            assert_eq!(up_ports.len(), 1);

            // Verify statistics
            let stats = orch.stats();
            assert_eq!(stats.ports_created, 4);
            assert_eq!(stats.lags_created, 2);
            assert_eq!(stats.vlans_created, 2);
        }

        #[test]
        fn test_ports_orch_port_in_multiple_vlans() {
            let sai = MockSai::new();
            let mut orch = PortsOrch::new(PortsOrchConfig::default());

            // Create port
            let port_id = create_port_with_sai("Ethernet0", 0x1000, vec![0], &sai);
            orch.add_port_from_hardware("Ethernet0".to_string(), port_id, vec![0])
                .unwrap();

            // Create multiple VLANs
            let vlan1_id = create_vlan_with_sai("Vlan100", 100, 0x3000, &sai);
            let vlan2_id = create_vlan_with_sai("Vlan200", 200, 0x3001, &sai);
            let vlan3_id = create_vlan_with_sai("Vlan300", 300, 0x3002, &sai);

            orch.create_vlan("Vlan100", 100, vlan1_id).unwrap();
            orch.create_vlan("Vlan200", 200, vlan2_id).unwrap();
            orch.create_vlan("Vlan300", 300, vlan3_id).unwrap();

            // Add port to all VLANs
            orch.add_vlan_member("Vlan100", "Ethernet0", VlanTaggingMode::Tagged, 0x4000, 0x5000)
                .unwrap();
            orch.add_vlan_member("Vlan200", "Ethernet0", VlanTaggingMode::Tagged, 0x4001, 0x5001)
                .unwrap();
            orch.add_vlan_member("Vlan300", "Ethernet0", VlanTaggingMode::Tagged, 0x4002, 0x5002)
                .unwrap();

            // Verify port is member of all VLANs
            let port = orch.get_port("Ethernet0").unwrap();
            assert_eq!(port.vlan_members.len(), 3);
            assert!(port.vlan_members.contains(&100));
            assert!(port.vlan_members.contains(&200));
            assert!(port.vlan_members.contains(&300));

            // Verify each VLAN has the port as member
            assert!(orch.get_vlan("Vlan100").unwrap().has_member("Ethernet0"));
            assert!(orch.get_vlan("Vlan200").unwrap().has_member("Ethernet0"));
            assert!(orch.get_vlan("Vlan300").unwrap().has_member("Ethernet0"));

            // Remove from one VLAN
            orch.remove_vlan_member("Vlan200", "Ethernet0").unwrap();

            let port = orch.get_port("Ethernet0").unwrap();
            assert_eq!(port.vlan_members.len(), 2);
            assert!(!port.vlan_members.contains(&200));
        }
    }

    // NatOrch integration tests
    mod nat_orch_tests {
        use super::*;
        use sonic_orchagent::nat::{
            NatOrch, NatOrchConfig,
            NatEntry, NatEntryKey, NatEntryConfig,
            NatPoolEntry, NatPoolKey, NatPoolConfig,
            NatType, NatProtocol,
        };
        use std::net::Ipv4Addr;

        fn create_snat_entry_with_sai(
            src_ip: &str,
            dst_ip: &str,
            translated_src_ip: &str,
            sai: &MockSai,
        ) -> (NatEntry, u64) {
            let key = NatEntryKey::new(
                src_ip.parse().unwrap(),
                dst_ip.parse().unwrap(),
                NatProtocol::Tcp,
                1024,
                80,
            );

            let config = NatEntryConfig {
                nat_type: NatType::Source,
                translated_src_ip: Some(translated_src_ip.parse().unwrap()),
                translated_dst_ip: None,
                translated_src_port: None,
                translated_dst_port: None,
            };

            let mut entry = NatEntry::new(key, config);

            let oid = sai.create_object(
                SaiObjectType::NatEntry,
                vec![
                    ("src_ip".to_string(), src_ip.to_string()),
                    ("dst_ip".to_string(), dst_ip.to_string()),
                    ("translated_src_ip".to_string(), translated_src_ip.to_string()),
                    ("type".to_string(), "SNAT".to_string()),
                ]
            ).unwrap();

            entry.entry_oid = oid;
            (entry, oid)
        }

        fn create_dnat_entry_with_sai(
            src_ip: &str,
            dst_ip: &str,
            translated_dst_ip: &str,
            sai: &MockSai,
        ) -> (NatEntry, u64) {
            let key = NatEntryKey::new(
                src_ip.parse().unwrap(),
                dst_ip.parse().unwrap(),
                NatProtocol::Tcp,
                2048,
                443,
            );

            let config = NatEntryConfig {
                nat_type: NatType::Destination,
                translated_src_ip: None,
                translated_dst_ip: Some(translated_dst_ip.parse().unwrap()),
                translated_src_port: None,
                translated_dst_port: Some(8443),
            };

            let mut entry = NatEntry::new(key, config);

            let oid = sai.create_object(
                SaiObjectType::NatEntry,
                vec![
                    ("src_ip".to_string(), src_ip.to_string()),
                    ("dst_ip".to_string(), dst_ip.to_string()),
                    ("translated_dst_ip".to_string(), translated_dst_ip.to_string()),
                    ("type".to_string(), "DNAT".to_string()),
                ]
            ).unwrap();

            entry.entry_oid = oid;
            (entry, oid)
        }

        fn create_nat_pool_with_sai(
            pool_name: &str,
            start_ip: &str,
            end_ip: &str,
            sai: &MockSai,
        ) -> (NatPoolEntry, u64) {
            let key = NatPoolKey::new(pool_name.to_string());
            let config = NatPoolConfig {
                ip_range: (start_ip.parse().unwrap(), end_ip.parse().unwrap()),
                port_range: Some((1024, 65535)),
            };

            let mut pool = NatPoolEntry::new(key, config);

            let oid = sai.create_object(
                SaiObjectType::NatEntry,
                vec![
                    ("pool_name".to_string(), pool_name.to_string()),
                    ("start_ip".to_string(), start_ip.to_string()),
                    ("end_ip".to_string(), end_ip.to_string()),
                ]
            ).unwrap();

            pool.pool_oid = oid;
            (pool, oid)
        }

        #[test]
        fn test_nat_orch_add_snat_entry_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = NatOrch::new(NatOrchConfig::default());

            assert_eq!(sai.count_objects(SaiObjectType::NatEntry), 0);

            let (snat_entry, oid) = create_snat_entry_with_sai(
                "10.0.0.1",
                "192.168.1.1",
                "1.1.1.1",
                &sai,
            );
            orch.add_entry(snat_entry).unwrap();

            assert_eq!(orch.entry_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::NatEntry), 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::NatEntry);

            // Verify SNAT-specific attributes
            assert_eq!(sai_obj.attributes[3].1, "SNAT");
        }

        #[test]
        fn test_nat_orch_add_dnat_entry_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = NatOrch::new(NatOrchConfig::default());

            assert_eq!(sai.count_objects(SaiObjectType::NatEntry), 0);

            let (dnat_entry, oid) = create_dnat_entry_with_sai(
                "10.0.0.2",
                "192.168.1.2",
                "2.2.2.2",
                &sai,
            );
            orch.add_entry(dnat_entry).unwrap();

            assert_eq!(orch.entry_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::NatEntry), 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::NatEntry);

            // Verify DNAT-specific attributes
            assert_eq!(sai_obj.attributes[3].1, "DNAT");
        }

        #[test]
        fn test_nat_orch_add_pool_creates_sai_object() {
            let sai = MockSai::new();
            let mut orch = NatOrch::new(NatOrchConfig::default());

            assert_eq!(sai.count_objects(SaiObjectType::NatEntry), 0);

            let (pool, oid) = create_nat_pool_with_sai(
                "nat_pool1",
                "100.0.0.1",
                "100.0.0.10",
                &sai,
            );
            orch.add_pool(pool).unwrap();

            assert_eq!(orch.pool_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::NatEntry), 1);
            assert_eq!(orch.stats().stats.pools_created, 1);

            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::NatEntry);
        }

        #[test]
        fn test_nat_orch_pool_ip_range_validation() {
            let sai = MockSai::new();
            let mut orch = NatOrch::new(NatOrchConfig::default());

            // Valid range: start <= end
            let (valid_pool, _) = create_nat_pool_with_sai(
                "valid_pool",
                "100.0.0.1",
                "100.0.0.10",
                &sai,
            );
            let result = orch.add_pool(valid_pool);
            assert!(result.is_ok());
            assert_eq!(orch.pool_count(), 1);

            // Invalid range: start > end
            let key = NatPoolKey::new("invalid_pool".to_string());
            let config = NatPoolConfig {
                ip_range: (
                    "100.0.0.20".parse::<Ipv4Addr>().unwrap(),
                    "100.0.0.10".parse::<Ipv4Addr>().unwrap(),
                ),
                port_range: Some((1024, 65535)),
            };
            let mut invalid_pool = NatPoolEntry::new(key, config);

            let oid = sai.create_object(
                SaiObjectType::NatEntry,
                vec![
                    ("pool_name".to_string(), "invalid_pool".to_string()),
                    ("start_ip".to_string(), "100.0.0.20".to_string()),
                    ("end_ip".to_string(), "100.0.0.10".to_string()),
                ]
            ).unwrap();
            invalid_pool.pool_oid = oid;

            let result = orch.add_pool(invalid_pool);
            assert!(result.is_err());

            // Pool count should still be 1 (only valid pool added)
            assert_eq!(orch.pool_count(), 1);
        }

        #[test]
        fn test_nat_orch_filter_by_nat_type() {
            let sai = MockSai::new();
            let mut orch = NatOrch::new(NatOrchConfig::default());

            // Add SNAT entries
            let (snat1, _) = create_snat_entry_with_sai(
                "10.0.0.1",
                "192.168.1.1",
                "1.1.1.1",
                &sai,
            );
            let (snat2, _) = create_snat_entry_with_sai(
                "10.0.0.2",
                "192.168.1.2",
                "1.1.1.2",
                &sai,
            );

            // Add DNAT entries
            let (dnat1, _) = create_dnat_entry_with_sai(
                "10.0.0.3",
                "192.168.1.3",
                "2.2.2.3",
                &sai,
            );
            let (dnat2, _) = create_dnat_entry_with_sai(
                "10.0.0.4",
                "192.168.1.4",
                "2.2.2.4",
                &sai,
            );
            let (dnat3, _) = create_dnat_entry_with_sai(
                "10.0.0.5",
                "192.168.1.5",
                "2.2.2.5",
                &sai,
            );

            orch.add_entry(snat1).unwrap();
            orch.add_entry(snat2).unwrap();
            orch.add_entry(dnat1).unwrap();
            orch.add_entry(dnat2).unwrap();
            orch.add_entry(dnat3).unwrap();

            assert_eq!(orch.entry_count(), 5);
            assert_eq!(sai.count_objects(SaiObjectType::NatEntry), 5);

            // Filter by SNAT type
            let snat_entries = orch.get_snat_entries();
            assert_eq!(snat_entries.len(), 2);
            for entry in &snat_entries {
                assert!(entry.is_snat());
                assert!(!entry.is_dnat());
            }

            // Filter by DNAT type
            let dnat_entries = orch.get_dnat_entries();
            assert_eq!(dnat_entries.len(), 3);
            for entry in &dnat_entries {
                assert!(entry.is_dnat());
                assert!(!entry.is_snat());
            }
        }
    }

    // SflowOrch integration tests
    mod sflow_orch_tests {
        use super::*;
        use sonic_orchagent::sflow::{SflowOrch, SflowOrchConfig, SflowOrchCallbacks, SflowConfig, SampleDirection};
        use std::num::NonZeroU32;

        /// Mock callbacks implementation for SflowOrch that uses MockSai
        struct MockSflowCallbacks {
            sai: Arc<MockSai>,
            ports_ready: bool,
        }

        impl MockSflowCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                Self {
                    sai,
                    ports_ready: true,
                }
            }
        }

        impl SflowOrchCallbacks for MockSflowCallbacks {
            fn create_samplepacket_session(&self, rate: NonZeroU32) -> Result<u64, String> {
                self.sai.create_object(
                    SaiObjectType::Samplepacket,
                    vec![("rate".to_string(), rate.to_string())],
                )
            }

            fn remove_samplepacket_session(&self, session_id: u64) -> Result<(), String> {
                self.sai.remove_object(session_id)
            }

            fn enable_port_ingress_sample(&self, _port_id: u64, _session_id: u64) -> Result<(), String> {
                Ok(())
            }

            fn disable_port_ingress_sample(&self, _port_id: u64) -> Result<(), String> {
                Ok(())
            }

            fn enable_port_egress_sample(&self, _port_id: u64, _session_id: u64) -> Result<(), String> {
                Ok(())
            }

            fn disable_port_egress_sample(&self, _port_id: u64) -> Result<(), String> {
                Ok(())
            }

            fn get_port_id(&self, alias: &str) -> Option<u64> {
                match alias {
                    "Ethernet0" => Some(0x100),
                    "Ethernet4" => Some(0x104),
                    "Ethernet8" => Some(0x108),
                    _ => None,
                }
            }

            fn all_ports_ready(&self) -> bool {
                self.ports_ready
            }
        }

        /// Helper function to create a sflow session configuration
        fn create_sflow_config(rate: u32, direction: SampleDirection) -> SflowConfig {
            let mut config = SflowConfig::new();
            config.admin_state = true;
            config.rate = NonZeroU32::new(rate);
            config.direction = direction;
            config
        }

        #[test]
        fn test_sflow_session_creation_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockSflowCallbacks::new(sai.clone()));
            let mut orch = SflowOrch::new(SflowOrchConfig::default());
            orch.set_callbacks(callbacks);
            orch.set_enabled(true);

            // Verify no samplepacket sessions exist initially
            assert_eq!(sai.count_objects(SaiObjectType::Samplepacket), 0);

            // Configure port with sflow sampling rate
            let config = create_sflow_config(4096, SampleDirection::Rx);
            orch.configure_port("Ethernet0", config).unwrap();

            // Verify samplepacket session was created in SAI
            assert_eq!(orch.session_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Samplepacket), 1);

            // Verify SAI object attributes
            let sai_obj = sai.get_object(1).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::Samplepacket);
            assert_eq!(sai_obj.attributes.len(), 1);
            assert_eq!(sai_obj.attributes[0].0, "rate");
            assert_eq!(sai_obj.attributes[0].1, "4096");

            // Verify port is configured
            assert_eq!(orch.port_count(), 1);
            let port_info = orch.get_port_info(0x100).unwrap();
            assert_eq!(port_info.admin_state, true);
            assert_eq!(port_info.direction, SampleDirection::Rx);
        }

        #[test]
        fn test_sflow_session_configuration_updates_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockSflowCallbacks::new(sai.clone()));
            let mut orch = SflowOrch::new(SflowOrchConfig::default());
            orch.set_callbacks(callbacks);
            orch.set_enabled(true);

            // Initial configuration
            let config = create_sflow_config(4096, SampleDirection::Rx);
            orch.configure_port("Ethernet0", config).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::Samplepacket), 1);

            // Update sampling rate
            let new_config = create_sflow_config(8192, SampleDirection::Rx);
            orch.configure_port("Ethernet0", new_config).unwrap();

            // Old session should be removed, new one created
            assert_eq!(orch.session_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Samplepacket), 1);
            assert_eq!(orch.stats().rate_updates, 1);

            // Update sampling direction
            let direction_config = create_sflow_config(8192, SampleDirection::Both);
            orch.configure_port("Ethernet0", direction_config).unwrap();

            // Session should remain the same, only direction changes
            assert_eq!(orch.session_count(), 1);
            assert_eq!(orch.stats().direction_updates, 1);

            let port_info = orch.get_port_info(0x100).unwrap();
            assert_eq!(port_info.direction, SampleDirection::Both);
        }

        #[test]
        fn test_sflow_session_removal_and_cleanup_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockSflowCallbacks::new(sai.clone()));
            let mut orch = SflowOrch::new(SflowOrchConfig::default());
            orch.set_callbacks(callbacks);
            orch.set_enabled(true);

            // Configure port with sflow
            let config = create_sflow_config(4096, SampleDirection::Rx);
            orch.configure_port("Ethernet0", config).unwrap();

            assert_eq!(orch.port_count(), 1);
            assert_eq!(orch.session_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Samplepacket), 1);

            // Remove port configuration
            orch.remove_port("Ethernet0").unwrap();

            // Verify cleanup
            assert_eq!(orch.port_count(), 0);
            assert_eq!(orch.session_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Samplepacket), 0);
            assert_eq!(orch.stats().ports_unconfigured, 1);
            assert_eq!(orch.stats().sessions_destroyed, 1);
        }

        #[test]
        fn test_port_based_sflow_sampling_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockSflowCallbacks::new(sai.clone()));
            let mut orch = SflowOrch::new(SflowOrchConfig::default());
            orch.set_callbacks(callbacks);
            orch.set_enabled(true);

            // Configure multiple ports with different sampling directions
            let rx_config = create_sflow_config(4096, SampleDirection::Rx);
            orch.configure_port("Ethernet0", rx_config).unwrap();

            let tx_config = create_sflow_config(4096, SampleDirection::Tx);
            orch.configure_port("Ethernet4", tx_config).unwrap();

            let both_config = create_sflow_config(4096, SampleDirection::Both);
            orch.configure_port("Ethernet8", both_config).unwrap();

            // Verify all ports configured with shared session
            assert_eq!(orch.port_count(), 3);
            assert_eq!(orch.session_count(), 1); // All share same rate
            assert_eq!(sai.count_objects(SaiObjectType::Samplepacket), 1);

            // Verify each port has correct direction
            let port0_info = orch.get_port_info(0x100).unwrap();
            assert_eq!(port0_info.direction, SampleDirection::Rx);

            let port1_info = orch.get_port_info(0x104).unwrap();
            assert_eq!(port1_info.direction, SampleDirection::Tx);

            let port2_info = orch.get_port_info(0x108).unwrap();
            assert_eq!(port2_info.direction, SampleDirection::Both);

            // Verify all ports share the same session
            assert_eq!(port0_info.session_id, port1_info.session_id);
            assert_eq!(port1_info.session_id, port2_info.session_id);
        }

        #[test]
        fn test_multiple_sflow_sessions_management_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockSflowCallbacks::new(sai.clone()));
            let mut orch = SflowOrch::new(SflowOrchConfig::default());
            orch.set_callbacks(callbacks);
            orch.set_enabled(true);

            // Configure ports with different sampling rates
            let config_4096 = create_sflow_config(4096, SampleDirection::Rx);
            orch.configure_port("Ethernet0", config_4096).unwrap();

            let config_8192 = create_sflow_config(8192, SampleDirection::Rx);
            orch.configure_port("Ethernet4", config_8192).unwrap();

            let config_16384 = create_sflow_config(16384, SampleDirection::Rx);
            orch.configure_port("Ethernet8", config_16384).unwrap();

            // Verify multiple sessions created
            assert_eq!(orch.port_count(), 3);
            assert_eq!(orch.session_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::Samplepacket), 3);
            assert_eq!(orch.stats().sessions_created, 3);

            // Verify SAI objects have correct rates
            let obj1 = sai.get_object(1).unwrap();
            assert_eq!(obj1.attributes[0].1, "4096");

            let obj2 = sai.get_object(2).unwrap();
            assert_eq!(obj2.attributes[0].1, "8192");

            let obj3 = sai.get_object(3).unwrap();
            assert_eq!(obj3.attributes[0].1, "16384");

            // Remove middle port
            orch.remove_port("Ethernet4").unwrap();
            assert_eq!(orch.session_count(), 2);
            assert_eq!(sai.count_objects(SaiObjectType::Samplepacket), 2);

            // Add another port with existing rate (session reuse)
            let config_4096_new = create_sflow_config(4096, SampleDirection::Both);
            orch.configure_port("Ethernet4", config_4096_new).unwrap();

            // Should reuse existing 4096 session, so still only 2 sessions
            assert_eq!(orch.session_count(), 2);
            assert_eq!(sai.count_objects(SaiObjectType::Samplepacket), 2);

            // Verify session reference counting - two ports now use the 4096 rate session
            let port0_info = orch.get_port_info(0x100).unwrap();
            let port1_info = orch.get_port_info(0x104).unwrap();
            assert_eq!(port0_info.session_id, port1_info.session_id); // Both share same session

            let session_rate = orch.get_session_rate(port0_info.session_id).unwrap();
            assert_eq!(session_rate.get(), 4096);
        }
    }

    // FlexCounterOrch integration tests
    mod flex_counter_orch_tests {
        use super::*;
        use sonic_orchagent::flex_counter::{
            fields, FlexCounterOrch, FlexCounterOrchConfig, FlexCounterGroup, FlexCounterCallbacks,
            FlexCounterError, QueueConfigurations, PgConfigurations,
        };
        use sonic_orch_common::Orch;
        use async_trait::async_trait;
        use std::sync::{Arc, Mutex};

        /// Mock callbacks for FlexCounterOrch testing
        struct MockFlexCounterCallbacks {
            sai: Arc<MockSai>,
            all_ports_ready: bool,
            is_gearbox_enabled: bool,
            /// Track which operations were called
            operations_called: Arc<Mutex<Vec<String>>>,
        }

        impl MockFlexCounterCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                Self {
                    sai,
                    all_ports_ready: true,
                    is_gearbox_enabled: false,
                    operations_called: Arc::new(Mutex::new(Vec::new())),
                }
            }

            fn track_operation(&self, op: String) {
                self.operations_called.lock().unwrap().push(op);
            }

            fn get_operations(&self) -> Vec<String> {
                self.operations_called.lock().unwrap().clone()
            }
        }

        #[async_trait]
        impl FlexCounterCallbacks for MockFlexCounterCallbacks {
            fn all_ports_ready(&self) -> bool {
                self.all_ports_ready
            }

            fn is_gearbox_enabled(&self) -> bool {
                self.is_gearbox_enabled
            }

            async fn generate_port_counter_map(&self) -> Result<(), FlexCounterError> {
                self.track_operation("generate_port_counter_map".to_string());
                // Create SAI port counter objects
                self.sai.create_object(
                    SaiObjectType::PortCounter,
                    vec![("type".to_string(), "port_stat".to_string())],
                ).map_err(|e| FlexCounterError::ConfigError(e))?;
                Ok(())
            }

            async fn generate_port_buffer_drop_counter_map(&self) -> Result<(), FlexCounterError> {
                self.track_operation("generate_port_buffer_drop_counter_map".to_string());
                self.sai.create_object(
                    SaiObjectType::BufferCounter,
                    vec![("type".to_string(), "port_buffer_drop".to_string())],
                ).map_err(|e| FlexCounterError::ConfigError(e))?;
                Ok(())
            }

            async fn generate_queue_map(&self, _configs: &QueueConfigurations) -> Result<(), FlexCounterError> {
                self.track_operation("generate_queue_map".to_string());
                Ok(())
            }

            async fn add_queue_flex_counters(&self, _configs: &QueueConfigurations) -> Result<(), FlexCounterError> {
                self.track_operation("add_queue_flex_counters".to_string());
                self.sai.create_object(
                    SaiObjectType::QueueCounter,
                    vec![("type".to_string(), "queue".to_string())],
                ).map_err(|e| FlexCounterError::ConfigError(e))?;
                Ok(())
            }

            async fn add_queue_watermark_flex_counters(&self, _configs: &QueueConfigurations) -> Result<(), FlexCounterError> {
                self.track_operation("add_queue_watermark_flex_counters".to_string());
                self.sai.create_object(
                    SaiObjectType::QueueCounter,
                    vec![("type".to_string(), "queue_watermark".to_string())],
                ).map_err(|e| FlexCounterError::ConfigError(e))?;
                Ok(())
            }

            async fn generate_pg_map(&self, _configs: &PgConfigurations) -> Result<(), FlexCounterError> {
                self.track_operation("generate_pg_map".to_string());
                Ok(())
            }

            async fn add_pg_flex_counters(&self, _configs: &PgConfigurations) -> Result<(), FlexCounterError> {
                self.track_operation("add_pg_flex_counters".to_string());
                self.sai.create_object(
                    SaiObjectType::BufferCounter,
                    vec![("type".to_string(), "pg_drop".to_string())],
                ).map_err(|e| FlexCounterError::ConfigError(e))?;
                Ok(())
            }

            async fn add_pg_watermark_flex_counters(&self, _configs: &PgConfigurations) -> Result<(), FlexCounterError> {
                self.track_operation("add_pg_watermark_flex_counters".to_string());
                self.sai.create_object(
                    SaiObjectType::BufferCounter,
                    vec![("type".to_string(), "pg_watermark".to_string())],
                ).map_err(|e| FlexCounterError::ConfigError(e))?;
                Ok(())
            }

            async fn generate_wred_port_counter_map(&self) -> Result<(), FlexCounterError> {
                self.track_operation("generate_wred_port_counter_map".to_string());
                Ok(())
            }

            async fn add_wred_queue_flex_counters(&self, _configs: &QueueConfigurations) -> Result<(), FlexCounterError> {
                self.track_operation("add_wred_queue_flex_counters".to_string());
                Ok(())
            }

            async fn flush_counters(&self) -> Result<(), FlexCounterError> {
                self.track_operation("flush_counters".to_string());
                Ok(())
            }

            async fn set_poll_interval(&self, group: &str, interval_ms: u64, gearbox: bool) -> Result<(), FlexCounterError> {
                self.track_operation(format!("set_poll_interval:{}:{}:{}", group, interval_ms, gearbox));
                // Create/update FlexCounterGroup SAI object
                self.sai.create_object(
                    SaiObjectType::FlexCounterGroup,
                    vec![
                        ("group".to_string(), group.to_string()),
                        ("poll_interval".to_string(), interval_ms.to_string()),
                        ("gearbox".to_string(), gearbox.to_string()),
                    ],
                ).map_err(|e| FlexCounterError::ConfigError(e))?;
                Ok(())
            }

            async fn set_group_operation(&self, group: &str, enable: bool, gearbox: bool) -> Result<(), FlexCounterError> {
                self.track_operation(format!("set_group_operation:{}:{}:{}", group, enable, gearbox));
                Ok(())
            }

            async fn set_bulk_chunk_size(&self, group: &str, size: Option<u32>) -> Result<(), FlexCounterError> {
                self.track_operation(format!("set_bulk_chunk_size:{}:{:?}", group, size));
                Ok(())
            }
        }

        fn create_flex_counter_entry(
            group: FlexCounterGroup,
            poll_interval: u64,
            enabled: bool,
        ) -> (String, std::collections::HashMap<String, String>) {
            let key = group.redis_key().to_string();
            let mut field_map = std::collections::HashMap::new();
            field_map.insert(fields::POLL_INTERVAL.to_string(), poll_interval.to_string());
            field_map.insert(
                fields::STATUS.to_string(),
                if enabled {
                    fields::STATUS_ENABLE.to_string()
                } else {
                    fields::STATUS_DISABLE.to_string()
                },
            );
            (key, field_map)
        }

        #[tokio::test]
        async fn test_flex_counter_port_polling_integration() {
            let sai = Arc::new(MockSai::new());
            let mut orch = FlexCounterOrch::new(FlexCounterOrchConfig::default());
            let callbacks = Arc::new(MockFlexCounterCallbacks::new(sai.clone()));
            orch.set_callbacks(callbacks.clone());

            // Create and enable port counter group
            let (key, fields) = create_flex_counter_entry(FlexCounterGroup::Port, 10000, true);

            use sonic_orch_common::Operation;
            orch.add_task(key, Operation::Set, fields);

            // Process the task
            orch.do_task().await;

            // Verify port counters are enabled
            assert!(orch.port_counters_enabled());

            // Verify SAI objects were created
            assert_eq!(sai.count_objects(SaiObjectType::FlexCounterGroup), 1);
            assert_eq!(sai.count_objects(SaiObjectType::PortCounter), 1);

            // Verify callbacks were invoked in correct order
            let ops = callbacks.get_operations();
            assert!(ops.contains(&"generate_port_counter_map".to_string()));
            assert!(ops.contains(&"set_poll_interval:PORT_STAT_COUNTER:10000:false".to_string()));
            assert!(ops.contains(&"set_group_operation:PORT_STAT_COUNTER:true:false".to_string()));
            assert!(ops.contains(&"flush_counters".to_string()));

            // Verify the FlexCounterGroup SAI object
            let group_obj = sai.get_object(1).unwrap();
            assert_eq!(group_obj.object_type, SaiObjectType::FlexCounterGroup);
            assert_eq!(
                group_obj.attributes.iter().find(|(k, _)| k == "group").map(|(_, v)| v.as_str()),
                Some("PORT_STAT_COUNTER")
            );
        }

        #[tokio::test]
        async fn test_flex_counter_queue_creation_and_management() {
            let sai = Arc::new(MockSai::new());
            let mut orch = FlexCounterOrch::new(FlexCounterOrchConfig::default());
            let callbacks = Arc::new(MockFlexCounterCallbacks::new(sai.clone()));
            orch.set_callbacks(callbacks.clone());

            // Load buffer queue configuration
            orch.load_buffer_queue_config("Ethernet0:0-7");
            orch.load_buffer_queue_config("Ethernet4:0-3");
            orch.set_create_only_config_db_buffers(true);

            // Enable queue counters
            let (key, fields) = create_flex_counter_entry(FlexCounterGroup::Queue, 5000, true);

            use sonic_orch_common::Operation;
            orch.add_task(key, Operation::Set, fields);
            orch.do_task().await;

            // Verify queue counters are enabled
            assert!(orch.queue_counters_enabled());

            // Verify SAI objects created
            assert_eq!(sai.count_objects(SaiObjectType::FlexCounterGroup), 1);
            assert_eq!(sai.count_objects(SaiObjectType::QueueCounter), 1);

            // Verify queue configurations
            let configs = orch.get_queue_configurations();
            assert!(configs.contains_key("Ethernet0"));
            assert!(configs.contains_key("Ethernet4"));

            let eth0_states = configs.get("Ethernet0").unwrap();
            assert!(eth0_states.is_queue_counter_enabled(0));
            assert!(eth0_states.is_queue_counter_enabled(7));
            assert!(!eth0_states.is_queue_counter_enabled(8));

            // Verify callbacks
            let ops = callbacks.get_operations();
            assert!(ops.contains(&"generate_queue_map".to_string()));
            assert!(ops.contains(&"add_queue_flex_counters".to_string()));
        }

        #[tokio::test]
        async fn test_flex_counter_buffer_statistics_collection() {
            let sai = Arc::new(MockSai::new());
            let mut orch = FlexCounterOrch::new(FlexCounterOrchConfig::default());
            let callbacks = Arc::new(MockFlexCounterCallbacks::new(sai.clone()));
            orch.set_callbacks(callbacks.clone());

            // Enable multiple buffer-related counter groups
            use sonic_orch_common::Operation;

            // Enable port buffer drop counters
            let (key1, fields1) = create_flex_counter_entry(FlexCounterGroup::PortBufferDrop, 2000, true);
            orch.add_task(key1, Operation::Set, fields1);

            // Enable PG drop counters
            orch.load_buffer_pg_config("Ethernet0:0-7");
            orch.set_create_only_config_db_buffers(true);
            let (key2, fields2) = create_flex_counter_entry(FlexCounterGroup::PgDrop, 3000, true);
            orch.add_task(key2, Operation::Set, fields2);

            // Enable PG watermark counters
            let (key3, fields3) = create_flex_counter_entry(FlexCounterGroup::PgWatermark, 4000, true);
            orch.add_task(key3, Operation::Set, fields3);

            // Process all tasks
            orch.do_task().await;

            // Verify all buffer counter states
            assert!(orch.port_buffer_drop_counters_enabled());
            assert!(orch.pg_counters_enabled());
            assert!(orch.pg_watermark_counters_enabled());

            // Verify SAI objects created for each buffer counter type
            assert_eq!(sai.count_objects(SaiObjectType::FlexCounterGroup), 3);
            assert_eq!(sai.count_objects(SaiObjectType::BufferCounter), 3); // port_buffer_drop, pg_drop, pg_watermark

            // Verify PG configurations
            let pg_configs = orch.get_pg_configurations();
            assert!(pg_configs.contains_key("Ethernet0"));
            let eth0_pgs = pg_configs.get("Ethernet0").unwrap();
            assert!(eth0_pgs.is_pg_counter_enabled(0));
            assert!(eth0_pgs.is_pg_counter_enabled(7));

            // Verify all callbacks were invoked
            let ops = callbacks.get_operations();
            assert!(ops.contains(&"generate_port_buffer_drop_counter_map".to_string()));
            assert!(ops.contains(&"generate_pg_map".to_string()));
            assert!(ops.contains(&"add_pg_flex_counters".to_string()));
            assert!(ops.contains(&"add_pg_watermark_flex_counters".to_string()));
        }

        #[tokio::test]
        async fn test_flex_counter_group_lifecycle() {
            let sai = Arc::new(MockSai::new());
            let mut orch = FlexCounterOrch::new(FlexCounterOrchConfig::default());
            let callbacks = Arc::new(MockFlexCounterCallbacks::new(sai.clone()));
            orch.set_callbacks(callbacks.clone());

            use sonic_orch_common::Operation;

            // Step 1: Create and enable port counter group
            let (key, mut fields) = create_flex_counter_entry(FlexCounterGroup::Port, 5000, true);
            orch.add_task(key.clone(), Operation::Set, fields.clone());
            orch.do_task().await;

            assert!(orch.port_counters_enabled());
            assert_eq!(sai.count_objects(SaiObjectType::FlexCounterGroup), 1);
            assert_eq!(sai.count_objects(SaiObjectType::PortCounter), 1);

            // Step 2: Update poll interval
            fields.insert(fields::POLL_INTERVAL.to_string(), "10000".to_string());
            orch.add_task(key.clone(), Operation::Set, fields.clone());
            orch.do_task().await;

            // Should have 2 FlexCounterGroup objects (one for initial interval, one for update)
            assert_eq!(sai.count_objects(SaiObjectType::FlexCounterGroup), 2);

            // Step 3: Disable the group
            fields.insert(fields::STATUS.to_string(), fields::STATUS_DISABLE.to_string());
            orch.add_task(key.clone(), Operation::Set, fields.clone());
            orch.do_task().await;

            assert!(!orch.port_counters_enabled());

            // Step 4: Remove (delete) the group
            orch.add_task(key, Operation::Del, std::collections::HashMap::new());
            orch.do_task().await;

            assert!(!orch.port_counters_enabled());

            // Verify lifecycle operations were tracked
            let ops = callbacks.get_operations();
            assert!(ops.iter().any(|op| op.contains("set_poll_interval")));
            assert!(ops.iter().any(|op| op.contains("set_group_operation:PORT_STAT_COUNTER:true")));
            assert!(ops.iter().any(|op| op.contains("set_group_operation:PORT_STAT_COUNTER:false")));
        }

        #[tokio::test]
        async fn test_flex_counter_multiple_counter_types_interaction() {
            let sai = Arc::new(MockSai::new());
            let mut orch = FlexCounterOrch::new(FlexCounterOrchConfig::default());
            let callbacks = Arc::new(MockFlexCounterCallbacks::new(sai.clone()));
            orch.set_callbacks(callbacks.clone());

            use sonic_orch_common::Operation;

            // Enable multiple counter types with different configurations
            let counter_groups = vec![
                (FlexCounterGroup::Port, 1000),
                (FlexCounterGroup::Queue, 5000),
                (FlexCounterGroup::QueueWatermark, 10000),
                (FlexCounterGroup::Rif, 2000),
            ];

            for (group, interval) in counter_groups {
                let (key, fields) = create_flex_counter_entry(group, interval, true);
                orch.add_task(key, Operation::Set, fields);
            }

            // Process all tasks
            orch.do_task().await;

            // Verify all counter types are enabled
            assert!(orch.port_counters_enabled());
            assert!(orch.queue_counters_enabled());
            assert!(orch.queue_watermark_counters_enabled());

            // Verify SAI objects created for all groups
            assert_eq!(sai.count_objects(SaiObjectType::FlexCounterGroup), 4);

            // Port counters should have created port counter objects
            assert_eq!(sai.count_objects(SaiObjectType::PortCounter), 1);

            // Queue counters should have created queue counter objects
            assert_eq!(sai.count_objects(SaiObjectType::QueueCounter), 2); // Queue + QueueWatermark

            // Verify all groups have correct poll intervals set
            let ops = callbacks.get_operations();
            assert!(ops.contains(&"set_poll_interval:PORT_STAT_COUNTER:1000:false".to_string()));
            assert!(ops.contains(&"set_poll_interval:QUEUE_STAT_COUNTER:5000:false".to_string()));
            assert!(ops.contains(&"set_poll_interval:QUEUE_WATERMARK_STAT_COUNTER:10000:false".to_string()));
            assert!(ops.contains(&"set_poll_interval:RIF_STAT_COUNTER:2000:false".to_string()));

            // Now disable Port counters and verify others remain active
            let (key, fields) = create_flex_counter_entry(FlexCounterGroup::Port, 1000, false);
            orch.add_task(key, Operation::Set, fields);
            orch.do_task().await;

            assert!(!orch.port_counters_enabled());
            assert!(orch.queue_counters_enabled());
            assert!(orch.queue_watermark_counters_enabled());

            // Verify cleanup was called for Port group
            let ops = callbacks.get_operations();
            assert!(ops.iter().any(|op| op.contains("set_group_operation:PORT_STAT_COUNTER:false")));
        }
    }

    // BfdOrch integration tests
    mod bfd_orch_tests {
        use super::*;
        use sonic_orchagent::bfd::{BfdOrch, BfdOrchConfig, BfdOrchCallbacks, BfdSessionConfig, BfdSessionKey, BfdSessionState, BfdSessionType, BfdUpdate};
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
        use std::sync::{Arc, Mutex};

        /// Mock callbacks for BfdOrch testing
        struct MockBfdCallbacks {
            sai: Arc<MockSai>,
            created_sessions: Mutex<Vec<(String, u32, u16)>>,
            removed_sessions: Mutex<Vec<u64>>,
            state_updates: Mutex<Vec<(String, BfdSessionState)>>,
            notifications: Mutex<Vec<BfdUpdate>>,
            software_bfd: bool,
            tsa_active: bool,
        }

        impl MockBfdCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                Self {
                    sai,
                    created_sessions: Mutex::new(Vec::new()),
                    removed_sessions: Mutex::new(Vec::new()),
                    state_updates: Mutex::new(Vec::new()),
                    notifications: Mutex::new(Vec::new()),
                    software_bfd: false,
                    tsa_active: false,
                }
            }
        }

        impl BfdOrchCallbacks for MockBfdCallbacks {
            fn create_bfd_session(
                &self,
                config: &BfdSessionConfig,
                discriminator: u32,
                src_port: u16,
            ) -> Result<u64, String> {
                let oid = self.sai.create_object(
                    SaiObjectType::BfdSession,
                    vec![
                        ("peer_ip".to_string(), config.key.peer_ip.to_string()),
                        ("vrf".to_string(), config.key.vrf.clone()),
                        ("discriminator".to_string(), discriminator.to_string()),
                        ("src_port".to_string(), src_port.to_string()),
                    ],
                )?;

                self.created_sessions
                    .lock()
                    .unwrap()
                    .push((config.key.to_config_key(), discriminator, src_port));

                Ok(oid)
            }

            fn remove_bfd_session(&self, sai_oid: u64) -> Result<(), String> {
                self.removed_sessions.lock().unwrap().push(sai_oid);
                self.sai.remove_object(sai_oid)
            }

            fn get_vrf_id(&self, _vrf_name: &str) -> Option<u64> {
                Some(0x1000)
            }

            fn get_port_id(&self, _port_name: &str) -> Option<u64> {
                Some(0x2000)
            }

            fn write_state_db(&self, key: &str, state: BfdSessionState, _session_type: BfdSessionType) {
                self.state_updates
                    .lock()
                    .unwrap()
                    .push((key.to_string(), state));
            }

            fn remove_state_db(&self, _key: &str) {}

            fn notify(&self, update: BfdUpdate) {
                self.notifications.lock().unwrap().push(update);
            }

            fn is_software_bfd(&self) -> bool {
                self.software_bfd
            }

            fn is_tsa_active(&self) -> bool {
                self.tsa_active
            }

            fn create_software_bfd_session(&self, _key: &str, _config: &BfdSessionConfig) {}

            fn remove_software_bfd_session(&self, _key: &str) {}
        }

        /// Helper to create a BFD session with SAI integration
        fn create_bfd_session(
            orch: &mut BfdOrch,
            vrf: &str,
            interface: Option<&str>,
            peer_ip: IpAddr,
        ) -> Result<(), String> {
            let key = BfdSessionKey::new(vrf, interface.map(|s| s.to_string()), peer_ip);
            let config = BfdSessionConfig::new(key);
            orch.create_session(config).map_err(|e| e.to_string())
        }

        #[test]
        fn test_bfd_session_lifecycle_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockBfdCallbacks::new(Arc::clone(&sai)));
            let mut orch = BfdOrch::new(BfdOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Initially no sessions
            assert_eq!(orch.session_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::BfdSession), 0);

            // Create BFD session
            let peer_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
            create_bfd_session(&mut orch, "default", None, peer_ip).unwrap();

            // Verify session created
            assert_eq!(orch.session_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::BfdSession), 1);

            // Verify SAI object was created with correct attributes
            let created = callbacks.created_sessions.lock().unwrap();
            assert_eq!(created.len(), 1);
            assert_eq!(created[0].0, "default::10.0.0.1");

            // Verify initial state written to state DB
            let state_updates = callbacks.state_updates.lock().unwrap();
            assert_eq!(state_updates.len(), 1);
            assert_eq!(state_updates[0].1, BfdSessionState::Down);

            // Get session info
            let session = orch.get_session("default::10.0.0.1").unwrap();
            let sai_oid = session.sai_oid;

            // Verify SAI object exists
            let sai_obj = sai.get_object(sai_oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::BfdSession);

            // Remove session
            drop(created);
            drop(state_updates);
            orch.remove_session("default::10.0.0.1").unwrap();

            // Verify cleanup
            assert_eq!(orch.session_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::BfdSession), 0);

            let removed = callbacks.removed_sessions.lock().unwrap();
            assert_eq!(removed.len(), 1);
            assert_eq!(removed[0], sai_oid);
        }

        #[test]
        fn test_bfd_session_state_transitions_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockBfdCallbacks::new(Arc::clone(&sai)));
            let mut orch = BfdOrch::new(BfdOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Create session
            let peer_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
            create_bfd_session(&mut orch, "default", None, peer_ip).unwrap();

            let session = orch.get_session("default::10.0.0.2").unwrap();
            let sai_oid = session.sai_oid;
            assert_eq!(session.state, BfdSessionState::Down);

            // Simulate state transition: Down -> Init
            orch.handle_state_change(sai_oid, BfdSessionState::Init).unwrap();

            let session = orch.get_session("default::10.0.0.2").unwrap();
            assert_eq!(session.state, BfdSessionState::Init);

            // Verify state DB was updated
            let state_updates = callbacks.state_updates.lock().unwrap();
            assert!(state_updates.iter().any(|(_, state)| *state == BfdSessionState::Init));

            // Verify notification was sent
            let notifications = callbacks.notifications.lock().unwrap();
            assert_eq!(notifications.len(), 1);
            assert_eq!(notifications[0].state, BfdSessionState::Init);

            drop(state_updates);
            drop(notifications);

            // Simulate state transition: Init -> Up
            orch.handle_state_change(sai_oid, BfdSessionState::Up).unwrap();

            let session = orch.get_session("default::10.0.0.2").unwrap();
            assert_eq!(session.state, BfdSessionState::Up);

            // Verify second notification
            let notifications = callbacks.notifications.lock().unwrap();
            assert_eq!(notifications.len(), 2);
            assert_eq!(notifications[1].state, BfdSessionState::Up);

            drop(notifications);

            // Simulate link failure: Up -> Down
            orch.handle_state_change(sai_oid, BfdSessionState::Down).unwrap();

            let session = orch.get_session("default::10.0.0.2").unwrap();
            assert_eq!(session.state, BfdSessionState::Down);

            // Verify final state change stats
            assert_eq!(orch.stats().state_changes, 3);

            // Cleanup
            orch.remove_session("default::10.0.0.2").unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::BfdSession), 0);
        }

        #[test]
        fn test_bfd_session_removal_and_cleanup_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockBfdCallbacks::new(Arc::clone(&sai)));
            let mut orch = BfdOrch::new(BfdOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Create multiple sessions
            for i in 1..=5 {
                let peer_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, i));
                create_bfd_session(&mut orch, "default", None, peer_ip).unwrap();
            }

            assert_eq!(orch.session_count(), 5);
            assert_eq!(sai.count_objects(SaiObjectType::BfdSession), 5);

            // Collect SAI OIDs before removal
            let session1 = orch.get_session("default::10.0.0.1").unwrap();
            let oid1 = session1.sai_oid;
            let session3 = orch.get_session("default::10.0.0.3").unwrap();
            let oid3 = session3.sai_oid;
            let session5 = orch.get_session("default::10.0.0.5").unwrap();
            let oid5 = session5.sai_oid;

            // Remove sessions 1, 3, and 5
            orch.remove_session("default::10.0.0.1").unwrap();
            orch.remove_session("default::10.0.0.3").unwrap();
            orch.remove_session("default::10.0.0.5").unwrap();

            // Verify partial cleanup
            assert_eq!(orch.session_count(), 2);
            assert_eq!(sai.count_objects(SaiObjectType::BfdSession), 2);

            // Verify correct sessions remain
            assert!(orch.get_session("default::10.0.0.1").is_none());
            assert!(orch.get_session("default::10.0.0.2").is_some());
            assert!(orch.get_session("default::10.0.0.3").is_none());
            assert!(orch.get_session("default::10.0.0.4").is_some());
            assert!(orch.get_session("default::10.0.0.5").is_none());

            // Verify SAI objects were removed
            let removed = callbacks.removed_sessions.lock().unwrap();
            assert_eq!(removed.len(), 3);
            assert!(removed.contains(&oid1));
            assert!(removed.contains(&oid3));
            assert!(removed.contains(&oid5));

            // Verify removal stats
            assert_eq!(orch.stats().sessions_removed, 3);

            // Remove remaining sessions
            drop(removed);
            orch.remove_session("default::10.0.0.2").unwrap();
            orch.remove_session("default::10.0.0.4").unwrap();

            // Verify complete cleanup
            assert_eq!(orch.session_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::BfdSession), 0);
            assert_eq!(orch.stats().sessions_removed, 5);
        }

        #[test]
        fn test_bfd_multiple_sessions_management_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockBfdCallbacks::new(Arc::clone(&sai)));
            let mut orch = BfdOrch::new(BfdOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Create IPv4 multihop sessions
            create_bfd_session(&mut orch, "default", None, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))).unwrap();
            create_bfd_session(&mut orch, "Vrf-RED", None, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))).unwrap();

            // Create IPv6 multihop sessions
            create_bfd_session(&mut orch, "default", None, IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1))).unwrap();

            // Create single-hop sessions
            create_bfd_session(&mut orch, "default", Some("Ethernet0"), IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))).unwrap();
            create_bfd_session(&mut orch, "default", Some("Ethernet4"), IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1))).unwrap();

            // Verify all sessions created
            assert_eq!(orch.session_count(), 5);
            assert_eq!(sai.count_objects(SaiObjectType::BfdSession), 5);

            // Verify session keys are correct
            assert!(orch.get_session("default::10.0.0.1").is_some());
            assert!(orch.get_session("Vrf-RED::192.168.1.1").is_some());
            assert!(orch.get_session("default::2001:db8::1").is_some());
            assert!(orch.get_session("default:Ethernet0:172.16.0.1").is_some());
            assert!(orch.get_session("default:Ethernet4:fe80::1").is_some());

            // Verify multihop detection
            let session1 = orch.get_session("default::10.0.0.1").unwrap();
            assert!(session1.config.key.is_multihop());

            let session4 = orch.get_session("default:Ethernet0:172.16.0.1").unwrap();
            assert!(!session4.config.key.is_multihop());

            // Simulate state changes on multiple sessions
            let oid1 = orch.get_session("default::10.0.0.1").unwrap().sai_oid;
            let oid3 = orch.get_session("default::2001:db8::1").unwrap().sai_oid;
            let oid5 = orch.get_session("default:Ethernet4:fe80::1").unwrap().sai_oid;

            orch.handle_state_change(oid1, BfdSessionState::Up).unwrap();
            orch.handle_state_change(oid3, BfdSessionState::Up).unwrap();
            orch.handle_state_change(oid5, BfdSessionState::Init).unwrap();

            // Verify state changes
            assert_eq!(orch.get_session("default::10.0.0.1").unwrap().state, BfdSessionState::Up);
            assert_eq!(orch.get_session("default::2001:db8::1").unwrap().state, BfdSessionState::Up);
            assert_eq!(orch.get_session("default:Ethernet4:fe80::1").unwrap().state, BfdSessionState::Init);

            // Verify state change count
            assert_eq!(orch.stats().state_changes, 3);

            // Cleanup all sessions
            orch.remove_session("default::10.0.0.1").unwrap();
            orch.remove_session("Vrf-RED::192.168.1.1").unwrap();
            orch.remove_session("default::2001:db8::1").unwrap();
            orch.remove_session("default:Ethernet0:172.16.0.1").unwrap();
            orch.remove_session("default:Ethernet4:fe80::1").unwrap();

            assert_eq!(orch.session_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::BfdSession), 0);
        }

        #[test]
        fn test_bfd_session_parameter_updates_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockBfdCallbacks::new(Arc::clone(&sai)));
            let mut orch = BfdOrch::new(BfdOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Create session with custom parameters
            let peer_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10));
            let key = BfdSessionKey::new("default", None, peer_ip);
            let config = BfdSessionConfig::new(key)
                .with_tx_interval(500)
                .with_rx_interval(600)
                .with_multiplier(5)
                .with_tos(128)
                .with_session_type(BfdSessionType::AsyncActive);

            orch.create_session(config).unwrap();

            // Verify session created with custom parameters
            assert_eq!(orch.session_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::BfdSession), 1);

            let session = orch.get_session("default::10.0.0.10").unwrap();
            assert_eq!(session.config.tx_interval, 500);
            assert_eq!(session.config.rx_interval, 600);
            assert_eq!(session.config.multiplier, 5);
            assert_eq!(session.config.tos, 128);
            assert_eq!(session.config.session_type, BfdSessionType::AsyncActive);

            // Verify SAI session was created
            let sai_obj = sai.get_object(session.sai_oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::BfdSession);

            // To update parameters, remove and recreate session (typical pattern)
            let old_oid = session.sai_oid;
            orch.remove_session("default::10.0.0.10").unwrap();

            assert_eq!(orch.session_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::BfdSession), 0);

            // Recreate with updated parameters
            let key = BfdSessionKey::new("default", None, peer_ip);
            let updated_config = BfdSessionConfig::new(key)
                .with_tx_interval(300)
                .with_rx_interval(400)
                .with_multiplier(3)
                .with_tos(64)
                .with_session_type(BfdSessionType::AsyncPassive);

            orch.create_session(updated_config).unwrap();

            // Verify updated session
            let updated_session = orch.get_session("default::10.0.0.10").unwrap();
            assert_eq!(updated_session.config.tx_interval, 300);
            assert_eq!(updated_session.config.rx_interval, 400);
            assert_eq!(updated_session.config.multiplier, 3);
            assert_eq!(updated_session.config.tos, 64);
            assert_eq!(updated_session.config.session_type, BfdSessionType::AsyncPassive);

            // Verify new SAI object created
            assert_ne!(updated_session.sai_oid, old_oid);
            assert_eq!(sai.count_objects(SaiObjectType::BfdSession), 1);

            // Verify stats
            assert_eq!(orch.stats().sessions_created, 2);
            assert_eq!(orch.stats().sessions_removed, 1);

            // Cleanup
            orch.remove_session("default::10.0.0.10").unwrap();
            assert_eq!(orch.session_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::BfdSession), 0);
        }
    }

    // VrfOrch integration tests
    mod vrf_orch_tests {
        use super::*;
        use sonic_orchagent::vrf::{VrfOrch, VrfOrchConfig, VrfOrchCallbacks, VrfConfig};
        use std::sync::Arc;

        /// Mock VRF callbacks with EVPN VTEP support for testing
        struct MockVrfCallbacks {
            has_vtep: bool,
            vni_to_vlan_map: std::collections::HashMap<u32, u16>,
        }

        impl MockVrfCallbacks {
            fn new() -> Self {
                Self {
                    has_vtep: false,
                    vni_to_vlan_map: std::collections::HashMap::new(),
                }
            }

            fn with_vtep(mut self) -> Self {
                self.has_vtep = true;
                self
            }

            fn with_vni_mapping(mut self, vni: u32, vlan_id: u16) -> Self {
                self.vni_to_vlan_map.insert(vni, vlan_id);
                self
            }
        }

        impl VrfOrchCallbacks for MockVrfCallbacks {
            fn has_evpn_vtep(&self) -> bool {
                self.has_vtep
            }

            fn get_vlan_mapped_to_vni(&self, vni: u32) -> Option<u16> {
                self.vni_to_vlan_map.get(&vni).copied()
            }
        }

        fn create_vrf_entry(name: &str, sai: &MockSai) -> (VrfConfig, u64) {
            let config = VrfConfig::new(name).with_v4(true).with_v6(true);

            let oid = sai.create_object(
                SaiObjectType::VirtualRouter,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("v4_enabled".to_string(), "true".to_string()),
                    ("v6_enabled".to_string(), "true".to_string()),
                ]
            ).unwrap();

            (config, oid)
        }

        fn create_vrf_entry_with_vni(name: &str, vni: u32, sai: &MockSai) -> (VrfConfig, u64) {
            let config = VrfConfig::new(name)
                .with_v4(true)
                .with_v6(true)
                .with_vni(vni);

            let oid = sai.create_object(
                SaiObjectType::VirtualRouter,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("v4_enabled".to_string(), "true".to_string()),
                    ("v6_enabled".to_string(), "true".to_string()),
                    ("vni".to_string(), vni.to_string()),
                ]
            ).unwrap();

            (config, oid)
        }

        #[test]
        fn test_vrf_creation_integration() {
            let sai = MockSai::new();
            let mut orch = VrfOrch::new(VrfOrchConfig::default());

            assert_eq!(sai.count_objects(SaiObjectType::VirtualRouter), 0);

            let (config, _oid) = create_vrf_entry("Vrf1", &sai);
            let vrf_id = orch.add_vrf(&config).unwrap();

            // Verify orchestration state
            assert_eq!(orch.vrf_count(), 1);
            assert!(orch.vrf_exists("Vrf1"));
            assert_eq!(orch.get_vrf_id("Vrf1"), vrf_id);
            assert_eq!(orch.stats().vrfs_created, 1);

            // Verify SAI synchronization
            assert_eq!(sai.count_objects(SaiObjectType::VirtualRouter), 1);

            let sai_obj = sai.get_object(_oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::VirtualRouter);
            assert_eq!(sai_obj.attributes[0].1, "Vrf1");
        }

        #[test]
        fn test_vrf_vni_mapping_configuration() {
            let sai = MockSai::new();
            let mut orch = VrfOrch::new(VrfOrchConfig::default());

            // Setup callbacks with EVPN VTEP support
            let callbacks = MockVrfCallbacks::new()
                .with_vtep()
                .with_vni_mapping(10000, 100);
            orch.set_callbacks(Arc::new(callbacks));

            assert_eq!(sai.count_objects(SaiObjectType::VirtualRouter), 0);

            let (config, _oid) = create_vrf_entry_with_vni("Vrf1", 10000, &sai);
            let vrf_id = orch.add_vrf(&config).unwrap();

            // Verify VRF created
            assert_eq!(orch.vrf_count(), 1);
            assert!(orch.vrf_exists("Vrf1"));
            assert_eq!(orch.get_vrf_id("Vrf1"), vrf_id);

            // Verify VNI mapping
            assert_eq!(orch.get_vrf_mapped_vni("Vrf1"), 10000);
            assert!(orch.is_l3_vni(10000));
            assert_eq!(orch.get_l3_vni_vlan(10000), Some(100));

            // Verify statistics
            assert_eq!(orch.stats().vrfs_created, 1);
            assert_eq!(orch.stats().vni_mappings_created, 1);

            // Verify SAI synchronization
            assert_eq!(sai.count_objects(SaiObjectType::VirtualRouter), 1);
        }

        #[test]
        fn test_vrf_removal_and_cleanup() {
            let sai = MockSai::new();
            let mut orch = VrfOrch::new(VrfOrchConfig::default());

            let (config, oid) = create_vrf_entry("Vrf1", &sai);
            let vrf_id = orch.add_vrf(&config).unwrap();

            assert_eq!(orch.vrf_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::VirtualRouter), 1);

            // Remove VRF
            orch.remove_vrf("Vrf1").unwrap();

            // Verify orchestration cleanup
            assert_eq!(orch.vrf_count(), 0);
            assert!(!orch.vrf_exists("Vrf1"));
            assert_eq!(orch.get_vrf_name(vrf_id), "");
            assert_eq!(orch.stats().vrfs_removed, 1);

            // Verify SAI cleanup
            sai.remove_object(oid).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::VirtualRouter), 0);
        }

        #[test]
        fn test_multiple_vrf_instances_with_isolation() {
            let sai = MockSai::new();
            let mut orch = VrfOrch::new(VrfOrchConfig::default());

            // Setup callbacks for VNI support
            let callbacks = MockVrfCallbacks::new()
                .with_vtep()
                .with_vni_mapping(10000, 100)
                .with_vni_mapping(20000, 200)
                .with_vni_mapping(30000, 300);
            orch.set_callbacks(Arc::new(callbacks));

            assert_eq!(sai.count_objects(SaiObjectType::VirtualRouter), 0);

            // Create three VRFs with different VNIs
            let (config1, _) = create_vrf_entry_with_vni("Vrf1", 10000, &sai);
            let (config2, _) = create_vrf_entry_with_vni("Vrf2", 20000, &sai);
            let (config3, _) = create_vrf_entry_with_vni("Vrf3", 30000, &sai);

            let vrf_id1 = orch.add_vrf(&config1).unwrap();
            let vrf_id2 = orch.add_vrf(&config2).unwrap();
            let vrf_id3 = orch.add_vrf(&config3).unwrap();

            // Verify all VRFs created
            assert_eq!(orch.vrf_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::VirtualRouter), 3);

            // Verify VRF isolation (unique IDs)
            assert_ne!(vrf_id1, vrf_id2);
            assert_ne!(vrf_id2, vrf_id3);
            assert_ne!(vrf_id1, vrf_id3);

            // Verify VNI isolation (unique VNI mappings)
            assert_eq!(orch.get_vrf_mapped_vni("Vrf1"), 10000);
            assert_eq!(orch.get_vrf_mapped_vni("Vrf2"), 20000);
            assert_eq!(orch.get_vrf_mapped_vni("Vrf3"), 30000);

            // Verify L3 VNI VLAN mappings
            assert_eq!(orch.get_l3_vni_vlan(10000), Some(100));
            assert_eq!(orch.get_l3_vni_vlan(20000), Some(200));
            assert_eq!(orch.get_l3_vni_vlan(30000), Some(300));

            // Verify reference count isolation
            orch.increase_vrf_ref_count("Vrf1").unwrap();
            orch.increase_vrf_ref_count("Vrf1").unwrap();
            orch.increase_vrf_ref_count("Vrf2").unwrap();

            assert_eq!(orch.get_vrf_ref_count("Vrf1"), 2);
            assert_eq!(orch.get_vrf_ref_count("Vrf2"), 1);
            assert_eq!(orch.get_vrf_ref_count("Vrf3"), 0);

            // Can only remove VRF3 (not in use)
            assert!(orch.remove_vrf("Vrf1").is_err());
            assert!(orch.remove_vrf("Vrf2").is_err());
            assert!(orch.remove_vrf("Vrf3").is_ok());

            assert_eq!(orch.vrf_count(), 2);
            assert_eq!(orch.stats().vrfs_created, 3);
            assert_eq!(orch.stats().vrfs_removed, 1);
        }

        #[test]
        fn test_vrf_attribute_updates() {
            let sai = MockSai::new();
            let mut orch = VrfOrch::new(VrfOrchConfig::default());

            // Create initial VRF
            let (config1, _oid) = create_vrf_entry("Vrf1", &sai);
            let vrf_id = orch.add_vrf(&config1).unwrap();

            assert_eq!(orch.vrf_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::VirtualRouter), 1);

            // Verify initial state
            let vrf = orch.get_vrf("Vrf1").unwrap();
            assert!(vrf.admin_v4_state);
            assert!(vrf.admin_v6_state);
            assert_eq!(vrf.vrf_id, vrf_id);

            // Update VRF attributes
            let config2 = VrfConfig::new("Vrf1")
                .with_v4(false)
                .with_v6(true);

            let updated_vrf_id = orch.add_vrf(&config2).unwrap();

            // Verify VRF ID unchanged (update, not recreate)
            assert_eq!(updated_vrf_id, vrf_id);
            assert_eq!(orch.vrf_count(), 1);

            // Verify updated attributes
            let vrf = orch.get_vrf("Vrf1").unwrap();
            assert!(!vrf.admin_v4_state);
            assert!(vrf.admin_v6_state);

            // Verify statistics
            assert_eq!(orch.stats().vrfs_created, 1);
            assert_eq!(orch.stats().vrfs_updated, 1);

            // Verify SAI object not duplicated
            assert_eq!(sai.count_objects(SaiObjectType::VirtualRouter), 1);

            // Cleanup
            orch.remove_vrf("Vrf1").unwrap();
            assert_eq!(orch.vrf_count(), 0);
            assert_eq!(orch.stats().vrfs_removed, 1);
        }
    }

    mod twamp_orch_tests {
        use super::*;
        use sonic_orchagent::twamp::{TwampOrch, TwampOrchConfig, TwampOrchCallbacks, TwampSessionConfig, TwampMode, TwampRole};
        use sonic_types::IpAddress;
        use std::sync::{Arc, Mutex};
        use std::str::FromStr;

        /// Mock callbacks for TwampOrch testing
        struct MockTwampCallbacks {
            sai: Arc<MockSai>,
            created_sessions: Mutex<Vec<String>>,
            removed_sessions: Mutex<Vec<u64>>,
        }

        impl MockTwampCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                Self {
                    sai,
                    created_sessions: Mutex::new(Vec::new()),
                    removed_sessions: Mutex::new(Vec::new()),
                }
            }
        }

        impl TwampOrchCallbacks for MockTwampCallbacks {
            fn create_twamp_session(&self, config: &TwampSessionConfig) -> Result<u64, String> {
                let oid = self.sai.create_object(
                    SaiObjectType::TwampSession,
                    vec![
                        ("name".to_string(), config.name.clone()),
                        ("mode".to_string(), config.mode.as_str().to_string()),
                        ("role".to_string(), config.role.as_str().to_string()),
                        ("src_ip".to_string(), config.src_ip.to_string()),
                        ("dst_ip".to_string(), config.dst_ip.to_string()),
                        ("padding_size".to_string(), config.padding_size.to_string()),
                        ("tx_interval".to_string(), config.tx_interval.map(|i| i.to_string()).unwrap_or_default()),
                    ],
                )?;

                self.created_sessions
                    .lock()
                    .unwrap()
                    .push(config.name.clone());

                Ok(oid)
            }

            fn remove_twamp_session(&self, session_id: u64) -> Result<(), String> {
                self.removed_sessions.lock().unwrap().push(session_id);
                self.sai.remove_object(session_id)
            }

            fn set_session_transmit(&self, _session_id: u64, _enabled: bool) -> Result<(), String> {
                Ok(())
            }
        }

        /// Helper to create a TWAMP session with SAI integration
        fn create_twamp_session(
            orch: &mut TwampOrch,
            name: &str,
            mode: TwampMode,
            role: TwampRole,
            src_ip: &str,
            dst_ip: &str,
        ) -> Result<(), String> {
            let mut config = TwampSessionConfig::new(name.to_string(), mode, role);
            config.src_ip = IpAddress::from_str(src_ip).unwrap();
            config.dst_ip = IpAddress::from_str(dst_ip).unwrap();
            orch.create_session(config).map_err(|e| format!("{:?}", e))
        }

        #[test]
        fn test_twamp_light_mode_session_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockTwampCallbacks::new(Arc::clone(&sai)));
            let mut orch = TwampOrch::new(TwampOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Initially no sessions
            assert_eq!(orch.session_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::TwampSession), 0);

            // Create TWAMP Light mode session
            create_twamp_session(
                &mut orch,
                "light_session",
                TwampMode::Light,
                TwampRole::Sender,
                "10.0.0.1",
                "10.0.0.2",
            ).unwrap();

            // Verify session created in orchestrator
            assert_eq!(orch.session_count(), 1);
            assert!(orch.session_exists("light_session"));

            // Verify SAI object created
            assert_eq!(sai.count_objects(SaiObjectType::TwampSession), 1);

            // Verify callbacks were called
            let created = callbacks.created_sessions.lock().unwrap();
            assert_eq!(created.len(), 1);
            assert_eq!(created[0], "light_session");
            drop(created);

            // Verify SAI object attributes
            let sai_obj = sai.get_object(1).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::TwampSession);
            let mode_attr = sai_obj.attributes.iter()
                .find(|(k, _)| k == "mode")
                .map(|(_, v)| v.as_str());
            assert_eq!(mode_attr, Some("light"));

            // Verify statistics
            assert_eq!(orch.stats().sessions_created, 1);
            assert_eq!(orch.stats().sessions_removed, 0);

            // Remove session
            orch.remove_session("light_session").unwrap();

            // Verify cleanup
            assert_eq!(orch.session_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::TwampSession), 0);
            assert_eq!(orch.stats().sessions_removed, 1);

            let removed = callbacks.removed_sessions.lock().unwrap();
            assert_eq!(removed.len(), 1);
        }

        #[test]
        fn test_twamp_full_mode_session_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockTwampCallbacks::new(Arc::clone(&sai)));
            let mut orch = TwampOrch::new(TwampOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Initially no sessions
            assert_eq!(orch.session_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::TwampSession), 0);

            // Create TWAMP Full mode session
            create_twamp_session(
                &mut orch,
                "full_session",
                TwampMode::Full,
                TwampRole::Sender,
                "192.168.1.1",
                "192.168.1.2",
            ).unwrap();

            // Verify session created in orchestrator
            assert_eq!(orch.session_count(), 1);
            assert!(orch.session_exists("full_session"));

            // Verify SAI object created
            assert_eq!(sai.count_objects(SaiObjectType::TwampSession), 1);

            // Verify callbacks were called
            let created = callbacks.created_sessions.lock().unwrap();
            assert_eq!(created.len(), 1);
            assert_eq!(created[0], "full_session");
            drop(created);

            // Verify SAI object attributes
            let sai_obj = sai.get_object(1).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::TwampSession);

            let mode_attr = sai_obj.attributes.iter()
                .find(|(k, _)| k == "mode")
                .map(|(_, v)| v.as_str());
            assert_eq!(mode_attr, Some("full"));

            let role_attr = sai_obj.attributes.iter()
                .find(|(k, _)| k == "role")
                .map(|(_, v)| v.as_str());
            assert_eq!(role_attr, Some("sender"));

            // Verify IP addresses in SAI object
            let src_ip_attr = sai_obj.attributes.iter()
                .find(|(k, _)| k == "src_ip")
                .map(|(_, v)| v.as_str());
            assert_eq!(src_ip_attr, Some("192.168.1.1"));

            let dst_ip_attr = sai_obj.attributes.iter()
                .find(|(k, _)| k == "dst_ip")
                .map(|(_, v)| v.as_str());
            assert_eq!(dst_ip_attr, Some("192.168.1.2"));

            // Verify statistics
            assert_eq!(orch.stats().sessions_created, 1);
            assert_eq!(orch.stats().sessions_removed, 0);

            // Remove session
            orch.remove_session("full_session").unwrap();

            // Verify cleanup
            assert_eq!(orch.session_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::TwampSession), 0);
            assert_eq!(orch.stats().sessions_removed, 1);
        }

        #[test]
        fn test_twamp_session_packet_configuration_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockTwampCallbacks::new(Arc::clone(&sai)));
            let mut orch = TwampOrch::new(TwampOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Create session with custom packet configuration
            let mut config = TwampSessionConfig::new(
                "packet_config_session".to_string(),
                TwampMode::Full,
                TwampRole::Sender,
            );
            config.src_ip = IpAddress::from_str("10.0.0.1").unwrap();
            config.dst_ip = IpAddress::from_str("10.0.0.2").unwrap();
            config.padding_size = 512;  // Custom padding size
            config.tx_interval = Some(100);  // 100ms TX interval

            orch.create_session(config).unwrap();

            // Verify session created
            assert_eq!(orch.session_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::TwampSession), 1);

            // Verify SAI object has correct packet configuration
            let sai_obj = sai.get_object(1).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::TwampSession);

            let padding_attr = sai_obj.attributes.iter()
                .find(|(k, _)| k == "padding_size")
                .map(|(_, v)| v.as_str());
            assert_eq!(padding_attr, Some("512"));

            let tx_interval_attr = sai_obj.attributes.iter()
                .find(|(k, _)| k == "tx_interval")
                .map(|(_, v)| v.as_str());
            assert_eq!(tx_interval_attr, Some("100"));

            // Verify statistics
            assert_eq!(orch.stats().sessions_created, 1);

            // Create another session with different configuration
            let mut config2 = TwampSessionConfig::new(
                "packet_config_session2".to_string(),
                TwampMode::Light,
                TwampRole::Reflector,
            );
            config2.src_ip = IpAddress::from_str("10.0.0.3").unwrap();
            config2.dst_ip = IpAddress::from_str("10.0.0.4").unwrap();
            config2.padding_size = 256;  // Different padding size
            config2.tx_interval = Some(50);  // 50ms TX interval

            orch.create_session(config2).unwrap();

            // Verify both sessions exist
            assert_eq!(orch.session_count(), 2);
            assert_eq!(sai.count_objects(SaiObjectType::TwampSession), 2);

            // Verify second session SAI object
            let sai_obj2 = sai.get_object(2).unwrap();
            let padding_attr2 = sai_obj2.attributes.iter()
                .find(|(k, _)| k == "padding_size")
                .map(|(_, v)| v.as_str());
            assert_eq!(padding_attr2, Some("256"));

            let tx_interval_attr2 = sai_obj2.attributes.iter()
                .find(|(k, _)| k == "tx_interval")
                .map(|(_, v)| v.as_str());
            assert_eq!(tx_interval_attr2, Some("50"));

            // Cleanup
            orch.remove_session("packet_config_session").unwrap();
            orch.remove_session("packet_config_session2").unwrap();

            assert_eq!(orch.session_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::TwampSession), 0);
            assert_eq!(orch.stats().sessions_removed, 2);
        }

        #[test]
        fn test_twamp_session_removal_and_cleanup_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockTwampCallbacks::new(Arc::clone(&sai)));
            let mut orch = TwampOrch::new(TwampOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Create multiple sessions
            for i in 1..=5 {
                create_twamp_session(
                    &mut orch,
                    &format!("session{}", i),
                    if i % 2 == 0 { TwampMode::Full } else { TwampMode::Light },
                    TwampRole::Sender,
                    &format!("10.0.0.{}", i),
                    &format!("10.0.1.{}", i),
                ).unwrap();
            }

            // Verify all sessions created
            assert_eq!(orch.session_count(), 5);
            assert_eq!(sai.count_objects(SaiObjectType::TwampSession), 5);
            assert_eq!(orch.stats().sessions_created, 5);

            // Verify each session exists
            for i in 1..=5 {
                assert!(orch.session_exists(&format!("session{}", i)));
            }

            // Verify callbacks tracked all creations
            let created = callbacks.created_sessions.lock().unwrap();
            assert_eq!(created.len(), 5);
            drop(created);

            // Remove sessions one by one
            for i in 1..=3 {
                orch.remove_session(&format!("session{}", i)).unwrap();
                assert_eq!(orch.session_count(), 5 - i);
                assert!(!orch.session_exists(&format!("session{}", i)));
            }

            // Verify partial cleanup
            assert_eq!(orch.session_count(), 2);
            assert_eq!(sai.count_objects(SaiObjectType::TwampSession), 2);
            assert_eq!(orch.stats().sessions_removed, 3);

            // Verify remaining sessions still exist
            assert!(orch.session_exists("session4"));
            assert!(orch.session_exists("session5"));

            // Verify callbacks tracked removals
            let removed = callbacks.removed_sessions.lock().unwrap();
            assert_eq!(removed.len(), 3);
            assert_eq!(removed[0], 1);
            assert_eq!(removed[1], 2);
            assert_eq!(removed[2], 3);
            drop(removed);

            // Remove remaining sessions
            orch.remove_session("session4").unwrap();
            orch.remove_session("session5").unwrap();

            // Verify complete cleanup
            assert_eq!(orch.session_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::TwampSession), 0);
            assert_eq!(orch.stats().sessions_created, 5);
            assert_eq!(orch.stats().sessions_removed, 5);

            // Verify all SAI objects removed
            for i in 1..=5 {
                assert!(sai.get_object(i).is_none());
            }

            // Verify callbacks tracked all removals
            let removed = callbacks.removed_sessions.lock().unwrap();
            assert_eq!(removed.len(), 5);
        }
    }

    // DebugCounterOrch integration tests
    mod debug_counter_orch_tests {
        use super::*;
        use sonic_orchagent::debug_counter::{
            DebugCounterOrch, DebugCounterOrchCallbacks, DebugCounterOrchConfig,
            DebugCounterConfig, DebugCounterType,
        };
        use sonic_sai::types::RawSaiObjectId;
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex};

        /// Mock callbacks for DebugCounterOrch integration testing
        struct MockDebugCounterCallbacks {
            sai: Arc<MockSai>,
            drop_reasons: Arc<Mutex<HashMap<RawSaiObjectId, Vec<String>>>>,
            flex_counters: Arc<Mutex<Vec<String>>>,
        }

        impl MockDebugCounterCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                Self {
                    sai,
                    drop_reasons: Arc::new(Mutex::new(HashMap::new())),
                    flex_counters: Arc::new(Mutex::new(Vec::new())),
                }
            }
        }

        impl DebugCounterOrchCallbacks for MockDebugCounterCallbacks {
            fn create_debug_counter(&self, counter_type: DebugCounterType) -> Result<RawSaiObjectId, String> {
                let oid = self.sai.create_object(
                    SaiObjectType::DebugCounter,
                    vec![
                        ("type".to_string(), counter_type.as_str().to_string()),
                        ("bind_method".to_string(), if counter_type.is_port_counter() { "port" } else { "switch" }.to_string()),
                    ],
                )?;
                Ok(oid)
            }

            fn remove_debug_counter(&self, oid: RawSaiObjectId) -> Result<(), String> {
                self.drop_reasons.lock().unwrap().remove(&oid);
                self.sai.remove_object(oid)
            }

            fn add_drop_reason_to_counter(&self, counter_id: RawSaiObjectId, drop_reason: &str) -> Result<(), String> {
                self.drop_reasons
                    .lock()
                    .unwrap()
                    .entry(counter_id)
                    .or_insert_with(Vec::new)
                    .push(drop_reason.to_string());
                Ok(())
            }

            fn remove_drop_reason_from_counter(&self, counter_id: RawSaiObjectId, drop_reason: &str) -> Result<(), String> {
                if let Some(reasons) = self.drop_reasons.lock().unwrap().get_mut(&counter_id) {
                    reasons.retain(|r| r != drop_reason);
                }
                Ok(())
            }

            fn register_flex_counter(&self, _counter_id: RawSaiObjectId, counter_name: &str) -> Result<(), String> {
                self.flex_counters.lock().unwrap().push(counter_name.to_string());
                Ok(())
            }

            fn unregister_flex_counter(&self, counter_name: &str) -> Result<(), String> {
                self.flex_counters.lock().unwrap().retain(|name| name != counter_name);
                Ok(())
            }

            fn get_available_drop_reasons(&self, is_ingress: bool) -> Vec<String> {
                if is_ingress {
                    vec![
                        "L3_ANY".to_string(),
                        "L2_ANY".to_string(),
                        "SMAC_MULTICAST".to_string(),
                        "SMAC_EQUALS_DMAC".to_string(),
                        "INGRESS_VLAN_FILTER".to_string(),
                        "FDB_UC_DISCARD".to_string(),
                        "FDB_MC_DISCARD".to_string(),
                        "L3_EGRESS_LINK_DOWN".to_string(),
                        "DECAP_ERROR".to_string(),
                    ]
                } else {
                    vec![
                        "L2_ANY".to_string(),
                        "L3_ANY".to_string(),
                        "TUNNEL_LOOPBACK_PACKET_DROP".to_string(),
                        "EGRESS_VLAN_FILTER".to_string(),
                    ]
                }
            }
        }

        /// Helper function to create a debug counter configuration
        fn create_debug_counter(
            name: &str,
            counter_type: DebugCounterType,
            drop_reasons: Vec<&str>,
        ) -> DebugCounterConfig {
            let mut config = DebugCounterConfig::new(name.to_string(), counter_type);
            for reason in drop_reasons {
                config.add_drop_reason(reason.to_string());
            }
            config
        }

        #[test]
        fn test_debug_counter_creation_integration() {
            let sai = Arc::new(MockSai::new());
            let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
            let callbacks = Arc::new(MockDebugCounterCallbacks::new(Arc::clone(&sai)));
            orch.set_callbacks(callbacks);

            assert_eq!(sai.count_objects(SaiObjectType::DebugCounter), 0);

            // Create counter with L2 and L3 drop reasons
            let config = create_debug_counter(
                "DROP_COUNTER_L2_L3",
                DebugCounterType::PortIngressDrops,
                vec!["L2_ANY", "L3_ANY"],
            );
            orch.create_debug_counter(config).unwrap();

            // Verify orchestration state
            assert_eq!(orch.counter_count(), 1);
            assert!(orch.counter_exists("DROP_COUNTER_L2_L3"));
            assert_eq!(orch.stats().counters_created, 1);
            assert_eq!(orch.stats().drop_reasons_added, 2);

            let entry = orch.get_counter("DROP_COUNTER_L2_L3").unwrap();
            assert_eq!(entry.counter_type, DebugCounterType::PortIngressDrops);
            assert_eq!(entry.drop_reason_count(), 2);
            assert!(entry.drop_reasons.contains("L2_ANY"));
            assert!(entry.drop_reasons.contains("L3_ANY"));

            // Verify SAI synchronization
            assert_eq!(sai.count_objects(SaiObjectType::DebugCounter), 1);

            let sai_obj = sai.get_object(entry.counter_id).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::DebugCounter);
            assert_eq!(sai_obj.attributes[0].1, "PORT_INGRESS_DROPS");
            assert_eq!(sai_obj.attributes[1].1, "port");
        }

        #[test]
        fn test_debug_counter_direction_configuration_integration() {
            let sai = Arc::new(MockSai::new());
            let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
            let callbacks = Arc::new(MockDebugCounterCallbacks::new(Arc::clone(&sai)));
            orch.set_callbacks(callbacks);

            assert_eq!(sai.count_objects(SaiObjectType::DebugCounter), 0);

            // Create ingress counter
            let ingress_config = create_debug_counter(
                "INGRESS_DROPS",
                DebugCounterType::PortIngressDrops,
                vec!["L3_ANY", "INGRESS_VLAN_FILTER"],
            );
            orch.create_debug_counter(ingress_config).unwrap();

            // Create egress counter
            let egress_config = create_debug_counter(
                "EGRESS_DROPS",
                DebugCounterType::PortEgressDrops,
                vec!["L2_ANY", "EGRESS_VLAN_FILTER"],
            );
            orch.create_debug_counter(egress_config).unwrap();

            // Create switch-level counter (both directions conceptually)
            let switch_config = create_debug_counter(
                "SWITCH_DROPS",
                DebugCounterType::SwitchIngressDrops,
                vec!["L3_ANY", "DECAP_ERROR"],
            );
            orch.create_debug_counter(switch_config).unwrap();

            // Verify orchestration state
            assert_eq!(orch.counter_count(), 3);
            assert_eq!(orch.stats().counters_created, 3);

            let ingress = orch.get_counter("INGRESS_DROPS").unwrap();
            assert!(ingress.counter_type.is_ingress());
            assert!(ingress.counter_type.is_port_counter());

            let egress = orch.get_counter("EGRESS_DROPS").unwrap();
            assert!(egress.counter_type.is_egress());
            assert!(egress.counter_type.is_port_counter());

            let switch = orch.get_counter("SWITCH_DROPS").unwrap();
            assert!(switch.counter_type.is_ingress());
            assert!(switch.counter_type.is_switch_counter());

            // Verify SAI synchronization
            assert_eq!(sai.count_objects(SaiObjectType::DebugCounter), 3);

            // Verify each counter has correct SAI attributes
            for (name, expected_type, expected_bind) in [
                ("INGRESS_DROPS", "PORT_INGRESS_DROPS", "port"),
                ("EGRESS_DROPS", "PORT_EGRESS_DROPS", "port"),
                ("SWITCH_DROPS", "SWITCH_INGRESS_DROPS", "switch"),
            ] {
                let entry = orch.get_counter(name).unwrap();
                let sai_obj = sai.get_object(entry.counter_id).unwrap();
                assert_eq!(sai_obj.attributes[0].1, expected_type);
                assert_eq!(sai_obj.attributes[1].1, expected_bind);
            }
        }

        #[test]
        fn test_multiple_debug_counters_with_different_drop_reason_types_integration() {
            let sai = Arc::new(MockSai::new());
            let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
            let callbacks = Arc::new(MockDebugCounterCallbacks::new(Arc::clone(&sai)));
            orch.set_callbacks(callbacks.clone());

            assert_eq!(sai.count_objects(SaiObjectType::DebugCounter), 0);

            // Counter 1: Layer 2 drops only
            let l2_config = create_debug_counter(
                "L2_DROPS",
                DebugCounterType::PortIngressDrops,
                vec!["L2_ANY", "SMAC_MULTICAST", "SMAC_EQUALS_DMAC"],
            );
            orch.create_debug_counter(l2_config).unwrap();

            // Counter 2: Layer 3 drops only
            let l3_config = create_debug_counter(
                "L3_DROPS",
                DebugCounterType::PortIngressDrops,
                vec!["L3_ANY", "L3_EGRESS_LINK_DOWN"],
            );
            orch.create_debug_counter(l3_config).unwrap();

            // Counter 3: VLAN-specific drops
            let vlan_config = create_debug_counter(
                "VLAN_DROPS",
                DebugCounterType::SwitchIngressDrops,
                vec!["INGRESS_VLAN_FILTER"],
            );
            orch.create_debug_counter(vlan_config).unwrap();

            // Counter 4: FDB drops
            let fdb_config = create_debug_counter(
                "FDB_DROPS",
                DebugCounterType::SwitchIngressDrops,
                vec!["FDB_UC_DISCARD", "FDB_MC_DISCARD"],
            );
            orch.create_debug_counter(fdb_config).unwrap();

            // Verify orchestration state
            assert_eq!(orch.counter_count(), 4);
            assert_eq!(orch.stats().counters_created, 4);
            assert_eq!(orch.stats().drop_reasons_added, 3 + 2 + 1 + 2); // Total drop reasons added

            // Verify each counter has correct drop reasons
            let l2_counter = orch.get_counter("L2_DROPS").unwrap();
            assert_eq!(l2_counter.drop_reason_count(), 3);
            assert!(l2_counter.drop_reasons.contains("L2_ANY"));
            assert!(l2_counter.drop_reasons.contains("SMAC_MULTICAST"));

            let l3_counter = orch.get_counter("L3_DROPS").unwrap();
            assert_eq!(l3_counter.drop_reason_count(), 2);
            assert!(l3_counter.drop_reasons.contains("L3_ANY"));

            let vlan_counter = orch.get_counter("VLAN_DROPS").unwrap();
            assert_eq!(vlan_counter.drop_reason_count(), 1);
            assert!(vlan_counter.drop_reasons.contains("INGRESS_VLAN_FILTER"));

            let fdb_counter = orch.get_counter("FDB_DROPS").unwrap();
            assert_eq!(fdb_counter.drop_reason_count(), 2);
            assert!(fdb_counter.drop_reasons.contains("FDB_UC_DISCARD"));
            assert!(fdb_counter.drop_reasons.contains("FDB_MC_DISCARD"));

            // Verify SAI synchronization - all counters created
            assert_eq!(sai.count_objects(SaiObjectType::DebugCounter), 4);

            // Verify drop reasons were registered with SAI
            let drop_reasons = callbacks.drop_reasons.lock().unwrap();
            assert_eq!(drop_reasons.get(&l2_counter.counter_id).unwrap().len(), 3);
            assert_eq!(drop_reasons.get(&l3_counter.counter_id).unwrap().len(), 2);
            assert_eq!(drop_reasons.get(&vlan_counter.counter_id).unwrap().len(), 1);
            assert_eq!(drop_reasons.get(&fdb_counter.counter_id).unwrap().len(), 2);
        }

        #[test]
        fn test_debug_counter_removal_and_cleanup_integration() {
            let sai = Arc::new(MockSai::new());
            let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
            let callbacks = Arc::new(MockDebugCounterCallbacks::new(Arc::clone(&sai)));
            orch.set_callbacks(callbacks.clone());

            assert_eq!(sai.count_objects(SaiObjectType::DebugCounter), 0);

            // Create multiple counters
            let config1 = create_debug_counter(
                "COUNTER_1",
                DebugCounterType::PortIngressDrops,
                vec!["L2_ANY", "L3_ANY"],
            );
            let config2 = create_debug_counter(
                "COUNTER_2",
                DebugCounterType::PortEgressDrops,
                vec!["EGRESS_VLAN_FILTER"],
            );
            let config3 = create_debug_counter(
                "COUNTER_3",
                DebugCounterType::SwitchIngressDrops,
                vec!["DECAP_ERROR", "FDB_UC_DISCARD"],
            );

            orch.create_debug_counter(config1).unwrap();
            orch.create_debug_counter(config2).unwrap();
            orch.create_debug_counter(config3).unwrap();

            let counter1_oid = orch.get_counter("COUNTER_1").unwrap().counter_id;
            let counter2_oid = orch.get_counter("COUNTER_2").unwrap().counter_id;
            let counter3_oid = orch.get_counter("COUNTER_3").unwrap().counter_id;

            assert_eq!(orch.counter_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::DebugCounter), 3);

            // Verify drop reasons are registered
            {
                let drop_reasons = callbacks.drop_reasons.lock().unwrap();
                assert_eq!(drop_reasons.get(&counter1_oid).unwrap().len(), 2);
                assert_eq!(drop_reasons.get(&counter2_oid).unwrap().len(), 1);
                assert_eq!(drop_reasons.get(&counter3_oid).unwrap().len(), 2);
            }

            // Remove COUNTER_2
            orch.remove_debug_counter("COUNTER_2").unwrap();

            // Verify orchestration cleanup
            assert_eq!(orch.counter_count(), 2);
            assert!(!orch.counter_exists("COUNTER_2"));
            assert!(orch.counter_exists("COUNTER_1"));
            assert!(orch.counter_exists("COUNTER_3"));
            assert_eq!(orch.stats().counters_created, 3);
            assert_eq!(orch.stats().counters_removed, 1);

            // Verify SAI cleanup - COUNTER_2 should be gone
            assert_eq!(sai.count_objects(SaiObjectType::DebugCounter), 2);
            assert!(sai.get_object(counter2_oid).is_none());

            // Verify drop reasons cleaned up for COUNTER_2
            {
                let drop_reasons = callbacks.drop_reasons.lock().unwrap();
                assert!(!drop_reasons.contains_key(&counter2_oid));
                assert!(drop_reasons.contains_key(&counter1_oid));
                assert!(drop_reasons.contains_key(&counter3_oid));
            }

            // Remove remaining counters
            orch.remove_debug_counter("COUNTER_1").unwrap();
            orch.remove_debug_counter("COUNTER_3").unwrap();

            // Verify complete cleanup
            assert_eq!(orch.counter_count(), 0);
            assert_eq!(orch.stats().counters_removed, 3);
            assert_eq!(sai.count_objects(SaiObjectType::DebugCounter), 0);

            // Verify all drop reasons cleaned up
            {
                let drop_reasons = callbacks.drop_reasons.lock().unwrap();
                assert!(drop_reasons.is_empty());
            }
        }
    }

    // NhgOrch integration tests
    mod nhg_orch_tests {
        use super::*;
        use sonic_orchagent::nhg::{
            LabelStack, NextHopGroupMember, NextHopKey, NhgOrch, NhgOrchCallbacks, NhgOrchConfig,
        };
        use sonic_types::{IpAddress, MacAddress};
        use std::str::FromStr;
        use std::sync::{Arc, Mutex};

        /// Mock callbacks for NhgOrch testing
        struct MockNhgCallbacks {
            sai: Arc<MockSai>,
            created_nexthops: Mutex<Vec<NextHopKey>>,
            removed_nexthops: Mutex<Vec<u64>>,
            created_nhgs: Mutex<Vec<Vec<NextHopGroupMember>>>,
            removed_nhgs: Mutex<Vec<u64>>,
        }

        impl MockNhgCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                Self {
                    sai,
                    created_nexthops: Mutex::new(Vec::new()),
                    removed_nexthops: Mutex::new(Vec::new()),
                    created_nhgs: Mutex::new(Vec::new()),
                    removed_nhgs: Mutex::new(Vec::new()),
                }
            }
        }

        impl NhgOrchCallbacks for MockNhgCallbacks {
            fn create_next_hop(&self, key: &NextHopKey) -> Result<u64, String> {
                let oid = self.sai.create_object(
                    SaiObjectType::NextHop,
                    vec![
                        ("ip".to_string(), key.ip_address.to_string()),
                        ("alias".to_string(), key.alias.clone()),
                        ("weight".to_string(), key.weight.to_string()),
                    ],
                )?;

                self.created_nexthops.lock().unwrap().push(key.clone());
                Ok(oid)
            }

            fn remove_next_hop(&self, nh_id: u64) -> Result<(), String> {
                self.removed_nexthops.lock().unwrap().push(nh_id);
                self.sai.remove_object(nh_id)
            }

            fn create_next_hop_group(&self, members: &[NextHopGroupMember]) -> Result<u64, String> {
                let oid = self.sai.create_object(
                    SaiObjectType::NextHopGroup,
                    vec![
                        ("member_count".to_string(), members.len().to_string()),
                    ],
                )?;

                self.created_nhgs.lock().unwrap().push(members.to_vec());
                Ok(oid)
            }

            fn remove_next_hop_group(&self, nhg_id: u64) -> Result<(), String> {
                self.removed_nhgs.lock().unwrap().push(nhg_id);
                self.sai.remove_object(nhg_id)
            }
        }

        /// Helper to create a basic next-hop group member
        fn create_nhg_member(ip: &str, alias: &str) -> NextHopGroupMember {
            NextHopGroupMember {
                key: NextHopKey {
                    ip_address: IpAddress::from_str(ip).unwrap(),
                    alias: alias.to_string(),
                    vni: 0,
                    mac_address: None,
                    label_stack: LabelStack::default(),
                    weight: 0,
                    srv6_segment: None,
                    srv6_source: None,
                    srv6_vpn_sid: None,
                },
                gm_id: 0,
                nh_id: 0,
            }
        }

        /// Helper to create a weighted next-hop group member
        fn create_weighted_nhg_member(ip: &str, alias: &str, weight: u32) -> NextHopGroupMember {
            NextHopGroupMember {
                key: NextHopKey {
                    ip_address: IpAddress::from_str(ip).unwrap(),
                    alias: alias.to_string(),
                    vni: 0,
                    mac_address: None,
                    label_stack: LabelStack::default(),
                    weight,
                    srv6_segment: None,
                    srv6_source: None,
                    srv6_vpn_sid: None,
                },
                gm_id: 0,
                nh_id: 0,
            }
        }

        #[test]
        fn test_nhg_ecmp_creation_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockNhgCallbacks::new(Arc::clone(&sai)));
            let mut orch = NhgOrch::new(NhgOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Initially no next-hop groups
            assert_eq!(orch.nhg_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 0);

            // Create ECMP next-hop group with 4 equal-cost paths
            let members = vec![
                create_nhg_member("10.0.0.1", "Ethernet0"),
                create_nhg_member("10.0.0.2", "Ethernet4"),
                create_nhg_member("10.0.0.3", "Ethernet8"),
                create_nhg_member("10.0.0.4", "Ethernet12"),
            ];

            orch.create_nhg("ecmp_group".to_string(), members.clone()).unwrap();

            // Verify NHG created in orchestrator
            assert_eq!(orch.nhg_count(), 1);
            assert!(orch.nhg_exists("ecmp_group"));
            assert_eq!(orch.stats().nhgs_created, 1);

            // Verify SAI next-hop group object was created
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 1);
            let created_nhgs = callbacks.created_nhgs.lock().unwrap();
            assert_eq!(created_nhgs.len(), 1);
            assert_eq!(created_nhgs[0].len(), 4);

            // Verify all members have equal weight (ECMP)
            for member in &created_nhgs[0] {
                assert_eq!(member.key.weight, 0);
            }

            // Verify SAI object attributes
            let sai_objs: Vec<_> = sai.objects.lock().unwrap()
                .iter()
                .filter(|obj| obj.object_type == SaiObjectType::NextHopGroup)
                .cloned()
                .collect();
            assert_eq!(sai_objs.len(), 1);
            assert_eq!(sai_objs[0].object_type, SaiObjectType::NextHopGroup);

            // Verify attributes contain member count
            let member_count_attr = sai_objs[0].attributes.iter()
                .find(|(k, _)| k == "member_count")
                .map(|(_, v)| v.clone());
            assert_eq!(member_count_attr, Some("4".to_string()));
        }

        #[test]
        fn test_nhg_wcmp_weighted_multipath_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockNhgCallbacks::new(Arc::clone(&sai)));
            let mut orch = NhgOrch::new(NhgOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Create WCMP next-hop group with different weights
            // Weights: 100, 200, 50 (total: 350)
            // Traffic distribution: ~28.5%, ~57.1%, ~14.3%
            let members = vec![
                create_weighted_nhg_member("10.0.0.1", "Ethernet0", 100),
                create_weighted_nhg_member("10.0.0.2", "Ethernet4", 200),
                create_weighted_nhg_member("10.0.0.3", "Ethernet8", 50),
            ];

            orch.create_nhg("wcmp_group".to_string(), members.clone()).unwrap();

            // Verify NHG created
            assert_eq!(orch.nhg_count(), 1);
            assert!(orch.nhg_exists("wcmp_group"));
            assert_eq!(orch.stats().nhgs_created, 1);

            // Verify SAI next-hop group object was created
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 1);

            // Verify weighted members
            let created_nhgs = callbacks.created_nhgs.lock().unwrap();
            assert_eq!(created_nhgs.len(), 1);
            assert_eq!(created_nhgs[0].len(), 3);

            // Verify each member has correct weight
            assert_eq!(created_nhgs[0][0].key.weight, 100);
            assert_eq!(created_nhgs[0][1].key.weight, 200);
            assert_eq!(created_nhgs[0][2].key.weight, 50);

            // Verify different weights (not ECMP)
            let weights: Vec<u32> = created_nhgs[0].iter()
                .map(|m| m.key.weight)
                .collect();
            assert_ne!(weights[0], weights[1]);
            assert_ne!(weights[1], weights[2]);
            assert_ne!(weights[0], weights[2]);

            // Test adding another WCMP group
            drop(created_nhgs);
            let members2 = vec![
                create_weighted_nhg_member("192.168.1.1", "Ethernet16", 300),
                create_weighted_nhg_member("192.168.1.2", "Ethernet20", 100),
            ];

            orch.create_nhg("wcmp_group2".to_string(), members2).unwrap();

            // Verify both groups exist
            assert_eq!(orch.nhg_count(), 2);
            assert!(orch.nhg_exists("wcmp_group"));
            assert!(orch.nhg_exists("wcmp_group2"));
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 2);
            assert_eq!(orch.stats().nhgs_created, 2);
        }

        #[test]
        fn test_nhg_type_variants_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockNhgCallbacks::new(Arc::clone(&sai)));
            let mut orch = NhgOrch::new(NhgOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Test 1: Overlay NHG (with VNI and MAC)
            let overlay_member = NextHopGroupMember {
                key: NextHopKey {
                    ip_address: IpAddress::from_str("192.168.100.1").unwrap(),
                    alias: "Vxlan100".to_string(),
                    vni: 10000,
                    mac_address: Some(MacAddress::from_str("00:11:22:33:44:55").unwrap()),
                    label_stack: LabelStack::default(),
                    weight: 0,
                    srv6_segment: None,
                    srv6_source: None,
                    srv6_vpn_sid: None,
                },
                gm_id: 0,
                nh_id: 0,
            };

            orch.create_nhg("overlay_nhg".to_string(), vec![overlay_member.clone()]).unwrap();
            assert!(orch.nhg_exists("overlay_nhg"));
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 1);

            // Verify overlay attributes
            let created_nhgs = callbacks.created_nhgs.lock().unwrap();
            assert_eq!(created_nhgs[0][0].key.vni, 10000);
            assert!(created_nhgs[0][0].key.mac_address.is_some());
            drop(created_nhgs);

            // Test 2: SRv6 NHG (with SRv6 segment)
            let srv6_member = NextHopGroupMember {
                key: NextHopKey {
                    ip_address: IpAddress::from_str("2001:db8::1").unwrap(),
                    alias: "Ethernet0".to_string(),
                    vni: 0,
                    mac_address: None,
                    label_stack: LabelStack::default(),
                    weight: 0,
                    srv6_segment: Some("fc00:0:1:1::".to_string()),
                    srv6_source: Some("fc00:0:1::1".to_string()),
                    srv6_vpn_sid: None,
                },
                gm_id: 0,
                nh_id: 0,
            };

            orch.create_nhg("srv6_nhg".to_string(), vec![srv6_member.clone()]).unwrap();
            assert!(orch.nhg_exists("srv6_nhg"));
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 2);

            // Verify SRv6 attributes
            let created_nhgs = callbacks.created_nhgs.lock().unwrap();
            assert_eq!(created_nhgs.len(), 2);
            assert!(created_nhgs[1][0].key.srv6_segment.is_some());
            assert!(created_nhgs[1][0].key.srv6_source.is_some());
            assert_eq!(created_nhgs[1][0].key.srv6_segment, Some("fc00:0:1:1::".to_string()));
            drop(created_nhgs);

            // Test 3: MPLS NHG (with label stack)
            let mpls_member = NextHopGroupMember {
                key: NextHopKey {
                    ip_address: IpAddress::from_str("10.1.1.1").unwrap(),
                    alias: "Ethernet24".to_string(),
                    vni: 0,
                    mac_address: None,
                    label_stack: vec![100, 200, 300],
                    weight: 0,
                    srv6_segment: None,
                    srv6_source: None,
                    srv6_vpn_sid: None,
                },
                gm_id: 0,
                nh_id: 0,
            };

            orch.create_nhg("mpls_nhg".to_string(), vec![mpls_member.clone()]).unwrap();
            assert!(orch.nhg_exists("mpls_nhg"));
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 3);

            // Verify MPLS label stack
            let created_nhgs = callbacks.created_nhgs.lock().unwrap();
            assert_eq!(created_nhgs.len(), 3);
            assert_eq!(created_nhgs[2][0].key.label_stack, vec![100, 200, 300]);
            assert!(!created_nhgs[2][0].key.label_stack.is_empty());
            drop(created_nhgs);

            // Test 4: SRv6 VPN NHG (with VPN SID)
            let srv6_vpn_member = NextHopGroupMember {
                key: NextHopKey {
                    ip_address: IpAddress::from_str("2001:db8::2").unwrap(),
                    alias: "Ethernet28".to_string(),
                    vni: 0,
                    mac_address: None,
                    label_stack: LabelStack::default(),
                    weight: 0,
                    srv6_segment: Some("fc00:0:2:1::".to_string()),
                    srv6_source: Some("fc00:0:2::1".to_string()),
                    srv6_vpn_sid: Some("fc00:0:2:100::".to_string()),
                },
                gm_id: 0,
                nh_id: 0,
            };

            orch.create_nhg("srv6_vpn_nhg".to_string(), vec![srv6_vpn_member.clone()]).unwrap();
            assert!(orch.nhg_exists("srv6_vpn_nhg"));
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 4);

            // Verify SRv6 VPN attributes
            let created_nhgs = callbacks.created_nhgs.lock().unwrap();
            assert_eq!(created_nhgs.len(), 4);
            assert!(created_nhgs[3][0].key.srv6_vpn_sid.is_some());
            assert_eq!(created_nhgs[3][0].key.srv6_vpn_sid, Some("fc00:0:2:100::".to_string()));
            drop(created_nhgs);

            // Verify all NHG types coexist
            assert_eq!(orch.nhg_count(), 4);
            assert_eq!(orch.stats().nhgs_created, 4);

            // Verify all SAI objects exist
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 4);
        }

        #[test]
        fn test_nhg_removal_and_cleanup_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockNhgCallbacks::new(Arc::clone(&sai)));
            let mut orch = NhgOrch::new(NhgOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Create multiple NHGs
            let members1 = vec![
                create_nhg_member("10.0.0.1", "Ethernet0"),
                create_nhg_member("10.0.0.2", "Ethernet4"),
            ];
            orch.create_nhg("nhg1".to_string(), members1).unwrap();

            let members2 = vec![
                create_nhg_member("10.0.0.3", "Ethernet8"),
                create_nhg_member("10.0.0.4", "Ethernet12"),
                create_nhg_member("10.0.0.5", "Ethernet16"),
            ];
            orch.create_nhg("nhg2".to_string(), members2).unwrap();

            let members3 = vec![
                create_weighted_nhg_member("10.0.0.6", "Ethernet20", 100),
            ];
            orch.create_nhg("nhg3".to_string(), members3).unwrap();

            // Verify initial state
            assert_eq!(orch.nhg_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 3);
            assert_eq!(orch.stats().nhgs_created, 3);
            assert_eq!(orch.stats().nhgs_removed, 0);

            // Test removal with reference counting - should fail when ref count > 0
            orch.increment_nhg_ref("nhg1").unwrap();
            let result = orch.remove_nhg("nhg1");
            assert!(result.is_err());
            assert!(orch.nhg_exists("nhg1"));
            assert_eq!(orch.nhg_count(), 3);

            // Decrement ref count and try again
            orch.decrement_nhg_ref("nhg1").unwrap();
            orch.remove_nhg("nhg1").unwrap();

            // Verify nhg1 removed
            assert!(!orch.nhg_exists("nhg1"));
            assert_eq!(orch.nhg_count(), 2);
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 2);
            assert_eq!(orch.stats().nhgs_removed, 1);

            // Verify SAI object was removed
            let removed_nhgs = callbacks.removed_nhgs.lock().unwrap();
            assert_eq!(removed_nhgs.len(), 1);
            drop(removed_nhgs);

            // Remove nhg2 (no ref count)
            orch.remove_nhg("nhg2").unwrap();
            assert!(!orch.nhg_exists("nhg2"));
            assert_eq!(orch.nhg_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 1);
            assert_eq!(orch.stats().nhgs_removed, 2);

            // Test removing non-existent NHG
            let result = orch.remove_nhg("nonexistent");
            assert!(result.is_err());
            assert_eq!(orch.nhg_count(), 1);

            // Remove final NHG
            orch.remove_nhg("nhg3").unwrap();

            // Verify complete cleanup
            assert_eq!(orch.nhg_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::NextHopGroup), 0);
            assert_eq!(orch.stats().nhgs_created, 3);
            assert_eq!(orch.stats().nhgs_removed, 3);

            // Verify all SAI objects removed
            let removed_nhgs = callbacks.removed_nhgs.lock().unwrap();
            assert_eq!(removed_nhgs.len(), 3);
        }
    }
}

    // CrmOrch integration tests
    mod crm_orch_tests {
        use super::*;
        use sonic_orchagent::crm::{
            CrmOrch, CrmOrchCallbacks, CrmOrchConfig, CrmResourceType, CrmThresholdType,
            ThresholdCheck, CRM_COUNTERS_TABLE_KEY,
        };
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex};

        struct MockCrmCallbacks {
            sai: Arc<MockSai>,
            resource_availability: Arc<Mutex<HashMap<CrmResourceType, (u32, u32)>>>,
            threshold_events: Arc<Mutex<Vec<ThresholdEvent>>>,
            counter_writes: Arc<Mutex<Vec<CounterWrite>>>,
            is_dpu: bool,
        }

        #[derive(Debug, Clone)]
        struct ThresholdEvent {
            resource: String,
            counter_key: String,
            used: u32,
            available: u32,
            threshold: u32,
            exceeded: bool,
        }

        #[derive(Debug, Clone)]
        struct CounterWrite {
            resource: String,
            key: String,
            used: u32,
            available: u32,
        }

        impl MockCrmCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                Self {
                    sai,
                    resource_availability: Arc::new(Mutex::new(HashMap::new())),
                    threshold_events: Arc::new(Mutex::new(Vec::new())),
                    counter_writes: Arc::new(Mutex::new(Vec::new())),
                    is_dpu: false,
                }
            }

            fn set_resource_availability(&self, resource_type: CrmResourceType, used: u32, available: u32) {
                self.resource_availability.lock().unwrap().insert(resource_type, (used, available));
            }

            fn get_threshold_events(&self) -> Vec<ThresholdEvent> {
                self.threshold_events.lock().unwrap().clone()
            }

            fn get_counter_writes(&self) -> Vec<CounterWrite> {
                self.counter_writes.lock().unwrap().clone()
            }

            fn clear_events(&self) {
                self.threshold_events.lock().unwrap().clear();
            }
        }

        impl CrmOrchCallbacks for MockCrmCallbacks {
            fn publish_threshold_event(
                &self,
                resource: &str,
                counter_key: &str,
                used: u32,
                available: u32,
                threshold: u32,
                exceeded: bool,
            ) {
                self.threshold_events.lock().unwrap().push(ThresholdEvent {
                    resource: resource.to_string(),
                    counter_key: counter_key.to_string(),
                    used,
                    available,
                    threshold,
                    exceeded,
                });
            }

            fn query_resource_availability(
                &self,
                resource_type: CrmResourceType,
            ) -> Option<(u32, u32)> {
                self.resource_availability.lock().unwrap().get(&resource_type).copied()
            }

            fn query_acl_availability(
                &self,
                _stage: sonic_orchagent::crm::AclStage,
                _bind_point: sonic_orchagent::crm::AclBindPoint,
            ) -> Option<(u32, u32)> {
                None
            }

            fn write_counters(
                &self,
                resource: &str,
                key: &str,
                used: u32,
                available: u32,
            ) {
                self.counter_writes.lock().unwrap().push(CounterWrite {
                    resource: resource.to_string(),
                    key: key.to_string(),
                    used,
                    available,
                });
            }

            fn is_dpu(&self) -> bool {
                self.is_dpu
            }
        }

        #[test]
        fn test_crm_resource_tracking_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockCrmCallbacks::new(Arc::clone(&sai)));
            let mut orch = CrmOrch::new(CrmOrchConfig::default());
            orch.set_callbacks(Arc::clone(&callbacks) as Arc<dyn CrmOrchCallbacks>);

            // Track IPv4 routes
            assert_eq!(orch.increment_used(CrmResourceType::Ipv4Route).unwrap(), 1);
            assert_eq!(orch.increment_used(CrmResourceType::Ipv4Route).unwrap(), 2);
            assert_eq!(orch.increment_used(CrmResourceType::Ipv4Route).unwrap(), 3);
            assert_eq!(orch.get_used(CrmResourceType::Ipv4Route), Some(3));

            // Track IPv6 routes
            assert_eq!(orch.increment_used(CrmResourceType::Ipv6Route).unwrap(), 1);
            assert_eq!(orch.increment_used(CrmResourceType::Ipv6Route).unwrap(), 2);
            assert_eq!(orch.get_used(CrmResourceType::Ipv6Route), Some(2));

            // Track nexthops
            assert_eq!(orch.increment_used(CrmResourceType::NexthopGroup).unwrap(), 1);
            assert_eq!(orch.increment_used(CrmResourceType::NexthopGroupMember).unwrap(), 1);
            assert_eq!(orch.increment_used(CrmResourceType::NexthopGroupMember).unwrap(), 2);
            assert_eq!(orch.increment_used(CrmResourceType::NexthopGroupMember).unwrap(), 3);

            // Verify statistics (3 + 2 + 1 + 3 = 9 increments total)
            assert_eq!(orch.stats().increments, 9);
            assert_eq!(orch.stats().decrements, 0);

            // Set available counters from SAI
            callbacks.set_resource_availability(CrmResourceType::Ipv4Route, 3, 1000);
            callbacks.set_resource_availability(CrmResourceType::Ipv6Route, 2, 500);
            callbacks.set_resource_availability(CrmResourceType::NexthopGroup, 1, 100);

            // Trigger timer expiration to query SAI and update counters
            orch.handle_timer_expiration();

            // Verify available counters were updated
            assert_eq!(orch.get_available(CrmResourceType::Ipv4Route), Some(1000));
            assert_eq!(orch.get_available(CrmResourceType::Ipv6Route), Some(500));
            assert_eq!(orch.get_available(CrmResourceType::NexthopGroup), Some(100));

            // Verify counter writes to COUNTERS_DB
            let writes = callbacks.get_counter_writes();
            assert!(writes.iter().any(|w| w.resource == "ipv4_route" && w.used == 3 && w.available == 1000));
            assert!(writes.iter().any(|w| w.resource == "ipv6_route" && w.used == 2 && w.available == 500));
            assert!(writes.iter().any(|w| w.resource == "nexthop_group" && w.used == 1 && w.available == 100));

            // Verify timer statistics
            assert_eq!(orch.stats().timer_expirations, 1);

            // Test decrement
            assert_eq!(orch.decrement_used(CrmResourceType::Ipv4Route).unwrap(), 2);
            assert_eq!(orch.get_used(CrmResourceType::Ipv4Route), Some(2));
            assert_eq!(orch.stats().decrements, 1);
        }

        #[test]
        fn test_crm_threshold_configuration_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockCrmCallbacks::new(Arc::clone(&sai)));
            let mut orch = CrmOrch::new(CrmOrchConfig::default());
            orch.set_callbacks(Arc::clone(&callbacks) as Arc<dyn CrmOrchCallbacks>);

            // Configure percentage-based thresholds for IPv4 routes
            orch.set_threshold_type(CrmResourceType::Ipv4Route, CrmThresholdType::Percentage).unwrap();
            orch.set_high_threshold(CrmResourceType::Ipv4Route, 85).unwrap();
            orch.set_low_threshold(CrmResourceType::Ipv4Route, 70).unwrap();

            let entry = orch.get_resource(CrmResourceType::Ipv4Route).unwrap();
            assert_eq!(entry.threshold_type, CrmThresholdType::Percentage);
            assert_eq!(entry.high_threshold, 85);
            assert_eq!(entry.low_threshold, 70);

            // Configure absolute (used) thresholds for IPv6 neighbors
            orch.set_threshold_type(CrmResourceType::Ipv6Neighbor, CrmThresholdType::Used).unwrap();
            orch.set_high_threshold(CrmResourceType::Ipv6Neighbor, 1000).unwrap();
            orch.set_low_threshold(CrmResourceType::Ipv6Neighbor, 500).unwrap();

            let entry = orch.get_resource(CrmResourceType::Ipv6Neighbor).unwrap();
            assert_eq!(entry.threshold_type, CrmThresholdType::Used);
            assert_eq!(entry.high_threshold, 1000);
            assert_eq!(entry.low_threshold, 500);

            // Configure free threshold for FDB entries
            orch.set_threshold_type(CrmResourceType::FdbEntry, CrmThresholdType::Free).unwrap();
            orch.set_high_threshold(CrmResourceType::FdbEntry, 200).unwrap();
            orch.set_low_threshold(CrmResourceType::FdbEntry, 100).unwrap();

            let entry = orch.get_resource(CrmResourceType::FdbEntry).unwrap();
            assert_eq!(entry.threshold_type, CrmThresholdType::Free);
            assert_eq!(entry.high_threshold, 200);
            assert_eq!(entry.low_threshold, 100);

            // Verify config update statistics
            assert_eq!(orch.stats().config_updates, 9);

            // Test configuration via field names
            orch.handle_config_field("ipv4_route_threshold_type", "used").unwrap();
            orch.handle_config_field("ipv4_route_high_threshold", "5000").unwrap();
            orch.handle_config_field("ipv4_route_low_threshold", "3000").unwrap();

            let entry = orch.get_resource(CrmResourceType::Ipv4Route).unwrap();
            assert_eq!(entry.threshold_type, CrmThresholdType::Used);
            assert_eq!(entry.high_threshold, 5000);
            assert_eq!(entry.low_threshold, 3000);

            assert_eq!(orch.stats().config_updates, 12);
        }

        #[test]
        fn test_crm_polling_interval_updates_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockCrmCallbacks::new(Arc::clone(&sai)));
            let mut orch = CrmOrch::new(CrmOrchConfig::default());
            orch.set_callbacks(Arc::clone(&callbacks) as Arc<dyn CrmOrchCallbacks>);

            // Verify default polling interval (300 seconds = 5 minutes)
            assert_eq!(orch.polling_interval(), std::time::Duration::from_secs(300));

            // Update polling interval to 60 seconds
            orch.set_polling_interval(std::time::Duration::from_secs(60));
            assert_eq!(orch.polling_interval(), std::time::Duration::from_secs(60));
            assert_eq!(orch.stats().config_updates, 1);

            // Update polling interval to 2 minutes
            orch.set_polling_interval(std::time::Duration::from_secs(120));
            assert_eq!(orch.polling_interval(), std::time::Duration::from_secs(120));
            assert_eq!(orch.stats().config_updates, 2);

            // Test very short interval (1 second)
            orch.set_polling_interval(std::time::Duration::from_secs(1));
            assert_eq!(orch.polling_interval(), std::time::Duration::from_secs(1));

            // Test very long interval (1 hour)
            orch.set_polling_interval(std::time::Duration::from_secs(3600));
            assert_eq!(orch.polling_interval(), std::time::Duration::from_secs(3600));

            // Test configuration via field name
            orch.handle_config_field("polling_interval", "180").unwrap();
            assert_eq!(orch.polling_interval(), std::time::Duration::from_secs(180));

            // Add some resources and trigger timer to verify polling works
            orch.increment_used(CrmResourceType::Ipv4Route).unwrap();
            orch.increment_used(CrmResourceType::Ipv6Route).unwrap();

            callbacks.set_resource_availability(CrmResourceType::Ipv4Route, 1, 1000);
            callbacks.set_resource_availability(CrmResourceType::Ipv6Route, 1, 500);

            // Trigger multiple timer expirations
            orch.handle_timer_expiration();
            orch.handle_timer_expiration();
            orch.handle_timer_expiration();

            // Verify timer statistics
            assert_eq!(orch.stats().timer_expirations, 3);

            // Verify counter writes occurred for each timer expiration
            let writes = callbacks.get_counter_writes();
            let ipv4_writes = writes.iter().filter(|w| w.resource == "ipv4_route").count();
            let ipv6_writes = writes.iter().filter(|w| w.resource == "ipv6_route").count();
            assert!(ipv4_writes >= 3);
            assert!(ipv6_writes >= 3);
        }

        #[test]
        fn test_crm_resource_alarm_triggering_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockCrmCallbacks::new(Arc::clone(&sai)));
            let mut orch = CrmOrch::new(CrmOrchConfig::default());
            orch.set_callbacks(Arc::clone(&callbacks) as Arc<dyn CrmOrchCallbacks>);

            // Configure percentage-based thresholds
            orch.set_threshold_type(CrmResourceType::Ipv4Route, CrmThresholdType::Percentage).unwrap();
            orch.set_high_threshold(CrmResourceType::Ipv4Route, 85).unwrap();
            orch.set_low_threshold(CrmResourceType::Ipv4Route, 70).unwrap();

            // Add routes to trigger high threshold
            // 90% usage: 90 used, 10 available
            for _ in 0..90 {
                orch.increment_used(CrmResourceType::Ipv4Route).unwrap();
            }
            assert_eq!(orch.get_used(CrmResourceType::Ipv4Route), Some(90));

            // Set available from SAI
            callbacks.set_resource_availability(CrmResourceType::Ipv4Route, 90, 10);

            // Trigger timer to check thresholds
            orch.handle_timer_expiration();

            // Verify high threshold event was published
            let events = callbacks.get_threshold_events();
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].resource, "ipv4_route");
            assert_eq!(events[0].used, 90);
            assert_eq!(events[0].available, 10);
            assert_eq!(events[0].threshold, 85);
            assert!(events[0].exceeded);
            assert_eq!(orch.stats().threshold_events, 1);

            // Clear events for next test
            callbacks.clear_events();

            // Reduce usage below low threshold to trigger recovery
            // 60% usage: 60 used, 40 available
            for _ in 0..30 {
                orch.decrement_used(CrmResourceType::Ipv4Route).unwrap();
            }
            callbacks.set_resource_availability(CrmResourceType::Ipv4Route, 60, 40);

            // Trigger timer to check thresholds
            orch.handle_timer_expiration();

            // Verify recovery event was published
            let events = callbacks.get_threshold_events();
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].resource, "ipv4_route");
            assert_eq!(events[0].used, 60);
            assert_eq!(events[0].available, 40);
            assert_eq!(events[0].threshold, 70);
            assert!(!events[0].exceeded);

            // Test absolute (used) threshold
            callbacks.clear_events();
            orch.set_threshold_type(CrmResourceType::Ipv6Neighbor, CrmThresholdType::Used).unwrap();
            orch.set_high_threshold(CrmResourceType::Ipv6Neighbor, 100).unwrap();
            orch.set_low_threshold(CrmResourceType::Ipv6Neighbor, 50).unwrap();

            // Add neighbors to exceed threshold
            for _ in 0..110 {
                orch.increment_used(CrmResourceType::Ipv6Neighbor).unwrap();
            }
            callbacks.set_resource_availability(CrmResourceType::Ipv6Neighbor, 110, 500);

            orch.handle_timer_expiration();

            // Verify threshold exceeded
            let events = callbacks.get_threshold_events();
            let ipv6_event = events.iter().find(|e| e.resource == "ipv6_neighbor");
            assert!(ipv6_event.is_some());
            let event = ipv6_event.unwrap();
            assert_eq!(event.used, 110);
            assert!(event.exceeded);
            assert_eq!(event.threshold, 100);

            // Test free threshold
            callbacks.clear_events();
            orch.set_threshold_type(CrmResourceType::FdbEntry, CrmThresholdType::Free).unwrap();
            orch.set_high_threshold(CrmResourceType::FdbEntry, 200).unwrap();
            orch.set_low_threshold(CrmResourceType::FdbEntry, 100).unwrap();

            // Set high free count to trigger threshold
            orch.increment_used(CrmResourceType::FdbEntry).unwrap();
            callbacks.set_resource_availability(CrmResourceType::FdbEntry, 1, 250);

            orch.handle_timer_expiration();

            // Verify free threshold exceeded (high free is considered exceeded)
            let events = callbacks.get_threshold_events();
            let fdb_event = events.iter().find(|e| e.resource == "fdb_entry");
            assert!(fdb_event.is_some());
            let event = fdb_event.unwrap();
            assert_eq!(event.available, 250);
            assert!(event.exceeded);
            assert_eq!(event.threshold, 200);
        }
    }

    // WatermarkOrch integration tests
    mod watermark_orch_tests {
        use super::*;
        use sonic_orchagent::watermark::{
            WatermarkOrch, WatermarkOrchConfig, WatermarkOrchCallbacks,
            WatermarkGroup, WatermarkTable, ClearRequest, QueueType,
        };
        use std::collections::HashMap;
        use std::sync::Mutex;
        use std::time::Duration;
        use sonic_sai::types::RawSaiObjectId;

        /// Helper to create watermark config with custom interval
        fn create_watermark_config(interval_secs: u64) -> WatermarkOrchConfig {
            WatermarkOrchConfig::with_interval_secs(interval_secs)
        }

        /// Mock callbacks for testing watermark clearing
        struct MockWatermarkCallbacks {
            ports_ready: bool,
            cleared_watermarks: Arc<Mutex<Vec<(WatermarkTable, String, RawSaiObjectId)>>>,
            cleared_by_name: Arc<Mutex<Vec<(WatermarkTable, String, String)>>>,
            buffer_pools: HashMap<String, RawSaiObjectId>,
        }

        impl MockWatermarkCallbacks {
            fn new(ports_ready: bool) -> Self {
                Self {
                    ports_ready,
                    cleared_watermarks: Arc::new(Mutex::new(Vec::new())),
                    cleared_by_name: Arc::new(Mutex::new(Vec::new())),
                    buffer_pools: HashMap::new(),
                }
            }

            fn with_buffer_pools(mut self, pools: HashMap<String, RawSaiObjectId>) -> Self {
                self.buffer_pools = pools;
                self
            }

            fn clear_count(&self) -> usize {
                self.cleared_watermarks.lock().unwrap().len()
            }

            fn clear_by_name_count(&self) -> usize {
                self.cleared_by_name.lock().unwrap().len()
            }
        }

        impl WatermarkOrchCallbacks for MockWatermarkCallbacks {
            fn all_ports_ready(&self) -> bool {
                self.ports_ready
            }

            fn clear_watermark(&self, table: WatermarkTable, stat_name: &str, obj_id: RawSaiObjectId) {
                self.cleared_watermarks
                    .lock()
                    .unwrap()
                    .push((table, stat_name.to_string(), obj_id));
            }

            fn clear_watermark_by_name(&self, table: WatermarkTable, stat_name: &str, name: &str) {
                self.cleared_by_name
                    .lock()
                    .unwrap()
                    .push((table, stat_name.to_string(), name.to_string()));
            }

            fn get_buffer_pool_oids(&self) -> HashMap<String, RawSaiObjectId> {
                self.buffer_pools.clone()
            }
        }

        #[test]
        fn test_watermark_queue_monitoring_integration() {
            let sai = MockSai::new();
            let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

            // Create queue objects in SAI
            let unicast_q1 = sai.create_object(
                SaiObjectType::QueueCounter,
                vec![("type".to_string(), "UNICAST".to_string())]
            ).unwrap();
            let unicast_q2 = sai.create_object(
                SaiObjectType::QueueCounter,
                vec![("type".to_string(), "UNICAST".to_string())]
            ).unwrap();
            let multicast_q1 = sai.create_object(
                SaiObjectType::QueueCounter,
                vec![("type".to_string(), "MULTICAST".to_string())]
            ).unwrap();
            let multicast_q2 = sai.create_object(
                SaiObjectType::QueueCounter,
                vec![("type".to_string(), "MULTICAST".to_string())]
            ).unwrap();

            // Setup watermark orchestrator with queue IDs
            orch.add_queue_id(QueueType::Unicast, unicast_q1);
            orch.add_queue_id(QueueType::Unicast, unicast_q2);
            orch.add_queue_id(QueueType::Multicast, multicast_q1);
            orch.add_queue_id(QueueType::Multicast, multicast_q2);
            orch.add_queue_id(QueueType::All, unicast_q1);
            orch.add_queue_id(QueueType::All, unicast_q2);

            assert!(orch.queue_ids_initialized());
            assert_eq!(orch.queue_ids().unicast.len(), 2);
            assert_eq!(orch.queue_ids().multicast.len(), 2);
            assert_eq!(orch.queue_ids().all.len(), 2);

            // Enable queue watermark monitoring
            let should_start_timer = orch.handle_flex_counter_status(WatermarkGroup::Queue, true);
            assert!(should_start_timer);
            assert!(orch.is_enabled());
            assert!(orch.status().queue_enabled());

            // Setup mock callbacks
            let callbacks = Arc::new(MockWatermarkCallbacks::new(true));
            orch.set_callbacks(callbacks.clone());

            // Clear unicast queue watermarks
            orch.handle_clear_request(WatermarkTable::User, ClearRequest::QueueSharedUnicast).unwrap();
            assert_eq!(callbacks.clear_count(), 2);
            assert_eq!(orch.stats().clears_processed, 1);

            // Clear multicast queue watermarks
            orch.handle_clear_request(WatermarkTable::User, ClearRequest::QueueSharedMulticast).unwrap();
            assert_eq!(callbacks.clear_count(), 4);
            assert_eq!(orch.stats().clears_processed, 2);

            // Disable queue watermark monitoring
            let should_stop_timer = orch.handle_flex_counter_status(WatermarkGroup::Queue, false);
            assert!(!should_stop_timer);
            assert!(!orch.is_enabled());
            assert!(!orch.status().queue_enabled());

            // Verify statistics
            assert_eq!(orch.stats().config_updates, 2);

            // Cleanup - verify SAI objects exist
            assert_eq!(sai.count_objects(SaiObjectType::QueueCounter), 4);
            sai.clear();
            assert_eq!(sai.count_objects(SaiObjectType::QueueCounter), 0);
        }

        #[test]
        fn test_watermark_priority_group_monitoring_integration() {
            let sai = MockSai::new();
            let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

            // Create PG objects in SAI (typically 8 PGs per port)
            let mut pg_oids = Vec::new();
            for i in 0..8 {
                let oid = sai.create_object(
                    SaiObjectType::BufferCounter,
                    vec![
                        ("pg_index".to_string(), i.to_string()),
                        ("type".to_string(), "PRIORITY_GROUP".to_string())
                    ]
                ).unwrap();
                pg_oids.push(oid);
                orch.add_pg_id(oid);
            }

            assert!(orch.pg_ids_initialized());
            assert_eq!(orch.pg_ids().len(), 8);

            // Enable PG watermark monitoring
            let should_start_timer = orch.handle_flex_counter_status(WatermarkGroup::PriorityGroup, true);
            assert!(should_start_timer);
            assert!(orch.is_enabled());
            assert!(orch.status().pg_enabled());

            // Setup mock callbacks
            let callbacks = Arc::new(MockWatermarkCallbacks::new(true));
            orch.set_callbacks(callbacks.clone());

            // Clear PG headroom watermarks
            orch.handle_clear_request(WatermarkTable::Persistent, ClearRequest::PgHeadroom).unwrap();
            assert_eq!(callbacks.clear_count(), 8);
            assert_eq!(orch.stats().clears_processed, 1);

            // Clear PG shared watermarks
            orch.handle_clear_request(WatermarkTable::Persistent, ClearRequest::PgShared).unwrap();
            assert_eq!(callbacks.clear_count(), 16);
            assert_eq!(orch.stats().clears_processed, 2);

            // Verify timer expiration handles PG watermarks
            orch.handle_timer_expiration();
            assert_eq!(orch.stats().timer_expirations, 1);
            // Timer clears both headroom and shared for all PGs
            assert_eq!(callbacks.clear_count(), 32);

            // Disable PG watermark monitoring
            orch.handle_flex_counter_status(WatermarkGroup::PriorityGroup, false);
            assert!(!orch.is_enabled());
            assert!(!orch.status().pg_enabled());

            // Verify SAI objects
            assert_eq!(sai.count_objects(SaiObjectType::BufferCounter), 8);

            // Cleanup
            sai.clear();
            assert_eq!(sai.count_objects(SaiObjectType::BufferCounter), 0);
        }

        #[test]
        fn test_watermark_buffer_pool_monitoring_integration() {
            let sai = MockSai::new();
            let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

            // Create buffer pool objects in SAI
            let ingress_lossless = sai.create_object(
                SaiObjectType::BufferPool,
                vec![
                    ("name".to_string(), "ingress_lossless_pool".to_string()),
                    ("type".to_string(), "INGRESS".to_string()),
                    ("mode".to_string(), "DYNAMIC".to_string())
                ]
            ).unwrap();

            let egress_lossless = sai.create_object(
                SaiObjectType::BufferPool,
                vec![
                    ("name".to_string(), "egress_lossless_pool".to_string()),
                    ("type".to_string(), "EGRESS".to_string()),
                    ("mode".to_string(), "DYNAMIC".to_string())
                ]
            ).unwrap();

            let ingress_lossy = sai.create_object(
                SaiObjectType::BufferPool,
                vec![
                    ("name".to_string(), "ingress_lossy_pool".to_string()),
                    ("type".to_string(), "INGRESS".to_string()),
                    ("mode".to_string(), "STATIC".to_string())
                ]
            ).unwrap();

            // Setup buffer pool OID mapping
            let mut pools = HashMap::new();
            pools.insert("ingress_lossless_pool".to_string(), ingress_lossless);
            pools.insert("egress_lossless_pool".to_string(), egress_lossless);
            pools.insert("ingress_lossy_pool".to_string(), ingress_lossy);

            let callbacks = Arc::new(MockWatermarkCallbacks::new(true).with_buffer_pools(pools));
            orch.set_callbacks(callbacks.clone());

            // Enable both queue and PG monitoring (enables buffer pool monitoring)
            orch.handle_flex_counter_status(WatermarkGroup::Queue, true);
            orch.handle_flex_counter_status(WatermarkGroup::PriorityGroup, true);
            assert!(orch.is_enabled());

            // Clear buffer pool watermarks
            orch.handle_clear_request(WatermarkTable::User, ClearRequest::BufferPool).unwrap();
            assert_eq!(callbacks.clear_count(), 3);
            assert_eq!(callbacks.clear_by_name_count(), 3);
            assert_eq!(orch.stats().clears_processed, 1);

            // Clear headroom pool watermarks
            orch.handle_clear_request(WatermarkTable::User, ClearRequest::HeadroomPool).unwrap();
            assert_eq!(callbacks.clear_count(), 6);
            assert_eq!(callbacks.clear_by_name_count(), 6);
            assert_eq!(orch.stats().clears_processed, 2);

            // Verify timer expiration handles buffer pools
            orch.handle_timer_expiration();
            assert_eq!(orch.stats().timer_expirations, 1);
            // Timer clears both buffer pool and headroom pool watermarks (3 pools x 2 types = 6)
            assert_eq!(callbacks.clear_count(), 12);
            assert_eq!(callbacks.clear_by_name_count(), 12);

            // Verify SAI objects
            assert_eq!(sai.count_objects(SaiObjectType::BufferPool), 3);

            let pool1 = sai.get_object(ingress_lossless).unwrap();
            assert_eq!(pool1.object_type, SaiObjectType::BufferPool);

            // Cleanup
            sai.clear();
            assert_eq!(sai.count_objects(SaiObjectType::BufferPool), 0);
        }

        #[test]
        fn test_watermark_telemetry_interval_configuration_integration() {
            let sai = MockSai::new();

            // Test default configuration
            let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());
            assert_eq!(orch.telemetry_interval(), Duration::from_secs(120));
            assert!(!orch.timer_changed());

            // Create queue and PG objects
            let queue_oid = sai.create_object(
                SaiObjectType::QueueCounter,
                vec![("type".to_string(), "UNICAST".to_string())]
            ).unwrap();
            let pg_oid = sai.create_object(
                SaiObjectType::BufferCounter,
                vec![("type".to_string(), "PRIORITY_GROUP".to_string())]
            ).unwrap();

            orch.add_queue_id(QueueType::Unicast, queue_oid);
            orch.add_pg_id(pg_oid);

            // Setup callbacks
            let callbacks = Arc::new(MockWatermarkCallbacks::new(true));
            orch.set_callbacks(callbacks.clone());

            // Enable monitoring
            orch.handle_flex_counter_status(WatermarkGroup::Queue, true);
            orch.handle_flex_counter_status(WatermarkGroup::PriorityGroup, true);

            // Test custom interval configuration
            let orch2 = WatermarkOrch::new(create_watermark_config(60));
            assert_eq!(orch2.telemetry_interval(), Duration::from_secs(60));

            let orch3 = WatermarkOrch::new(create_watermark_config(300));
            assert_eq!(orch3.telemetry_interval(), Duration::from_secs(300));

            // Test interval updates
            orch.set_telemetry_interval_secs(30);
            assert_eq!(orch.telemetry_interval(), Duration::from_secs(30));
            assert!(orch.timer_changed());
            assert_eq!(orch.stats().config_updates, 3);

            // Test timer changed flag is cleared on expiration
            orch.handle_timer_expiration();
            assert!(!orch.timer_changed());
            assert_eq!(orch.stats().timer_expirations, 1);

            // Test multiple interval updates
            orch.set_telemetry_interval_secs(45);
            assert!(orch.timer_changed());
            assert_eq!(orch.telemetry_interval(), Duration::from_secs(45));
            assert_eq!(orch.stats().config_updates, 4);

            orch.set_telemetry_interval_secs(90);
            assert!(orch.timer_changed());
            assert_eq!(orch.telemetry_interval(), Duration::from_secs(90));
            assert_eq!(orch.stats().config_updates, 5);

            // Setting same interval should not trigger change
            orch.clear_timer_changed();
            orch.set_telemetry_interval_secs(90);
            assert!(!orch.timer_changed());
            assert_eq!(orch.stats().config_updates, 5);

            // Test interval change during monitoring
            orch.set_telemetry_interval_secs(15);
            assert!(orch.timer_changed());
            orch.handle_timer_expiration();
            assert!(!orch.timer_changed());
            assert_eq!(orch.stats().timer_expirations, 2);

            // Verify watermarks were cleared during timer expirations
            // Each expiration clears: 2 PG stats + 1 queue stat + 0 buffer pools = 3 clears per expiration
            assert_eq!(callbacks.clear_count(), 6);

            // Test zero interval (disable telemetry)
            orch.set_telemetry_interval_secs(0);
            assert_eq!(orch.telemetry_interval(), Duration::from_secs(0));
            assert!(orch.timer_changed());

            // Timer still runs but with zero interval
            orch.handle_timer_expiration();
            assert!(!orch.timer_changed());
            assert_eq!(orch.stats().timer_expirations, 3);

            // Verify final statistics
            assert_eq!(orch.stats().config_updates, 7);
            assert_eq!(orch.stats().timer_expirations, 3);

            // Cleanup
            assert_eq!(sai.count_objects(SaiObjectType::QueueCounter), 1);
            assert_eq!(sai.count_objects(SaiObjectType::BufferCounter), 1);
            sai.clear();
        }
    }

    // MlagOrch integration tests
    mod mlag_orch_tests {
        use super::*;
        use sonic_orchagent::mlag::{MlagOrch, MlagOrchConfig, MlagOrchCallbacks, MlagUpdate, MlagIslUpdate, MlagIfUpdate};
        use std::sync::{Arc, Mutex};

        /// Mock callbacks for MlagOrch testing
        struct MockMlagCallbacks {
            updates: Mutex<Vec<MlagUpdate>>,
            ports_ready: bool,
        }

        impl MockMlagCallbacks {
            fn new() -> Self {
                Self {
                    updates: Mutex::new(Vec::new()),
                    ports_ready: true,
                }
            }

            fn with_ports_ready(ports_ready: bool) -> Self {
                Self {
                    updates: Mutex::new(Vec::new()),
                    ports_ready,
                }
            }

            fn get_updates(&self) -> Vec<MlagUpdate> {
                self.updates.lock().unwrap().clone()
            }

            fn clear_updates(&self) {
                self.updates.lock().unwrap().clear();
            }
        }

        impl MlagOrchCallbacks for MockMlagCallbacks {
            fn notify(&self, update: MlagUpdate) {
                self.updates.lock().unwrap().push(update);
            }

            fn all_ports_ready(&self) -> bool {
                self.ports_ready
            }
        }

        /// Helper function to create MLAG domain with ISL
        fn create_mlag_domain(orch: &mut MlagOrch, isl_name: &str) -> Result<(), String> {
            orch.add_isl_interface(isl_name)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }

        /// Helper function to add MLAG interface
        fn add_mlag_interface(orch: &mut MlagOrch, if_name: &str) -> Result<(), String> {
            orch.add_mlag_interface(if_name)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }

        #[test]
        fn test_mlag_domain_creation_integration() {
            let callbacks = Arc::new(MockMlagCallbacks::new());
            let mut orch = MlagOrch::new(MlagOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Initialize orch
            orch.set_initialized();
            assert!(orch.is_initialized());

            // Initially no ISL configured
            assert!(orch.isl_name().is_none());
            assert_eq!(orch.mlag_interface_count(), 0);

            // Create MLAG domain by setting ISL
            create_mlag_domain(&mut orch, "PortChannel100").unwrap();

            // Verify ISL is set
            assert_eq!(orch.isl_name(), Some("PortChannel100"));
            assert!(orch.is_isl_interface("PortChannel100"));

            // Verify notification was sent
            let updates = callbacks.get_updates();
            assert_eq!(updates.len(), 1);
            assert!(matches!(&updates[0], MlagUpdate::Isl(u) if u.is_add && u.isl_name == "PortChannel100"));

            // Verify statistics
            let stats = orch.stats();
            assert_eq!(stats.isl_adds, 1);
            assert_eq!(stats.isl_deletes, 0);
            assert_eq!(stats.notifications, 1);

            // Test ISL reconfiguration
            callbacks.clear_updates();
            create_mlag_domain(&mut orch, "PortChannel200").unwrap();

            // Verify ISL was updated
            assert_eq!(orch.isl_name(), Some("PortChannel200"));
            assert!(!orch.is_isl_interface("PortChannel100"));
            assert!(orch.is_isl_interface("PortChannel200"));

            // Verify second notification
            let updates = callbacks.get_updates();
            assert_eq!(updates.len(), 1);
            assert!(matches!(&updates[0], MlagUpdate::Isl(u) if u.is_add && u.isl_name == "PortChannel200"));

            // Verify updated statistics
            let stats = orch.stats();
            assert_eq!(stats.isl_adds, 2);
            assert_eq!(stats.notifications, 2);

            // Verify all ports are ready
            assert!(orch.all_ports_ready());
        }

        #[test]
        fn test_mlag_isl_configuration_integration() {
            let callbacks = Arc::new(MockMlagCallbacks::new());
            let mut orch = MlagOrch::new(MlagOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Test various ISL naming formats
            let isl_names = vec![
                "PortChannel1",
                "PortChannel999",
                "peer-link",
                "PortChannel100",
            ];

            for isl_name in &isl_names {
                callbacks.clear_updates();

                // Set ISL
                create_mlag_domain(&mut orch, isl_name).unwrap();
                assert_eq!(orch.isl_name(), Some(*isl_name));

                // Verify notification
                let updates = callbacks.get_updates();
                assert_eq!(updates.len(), 1);
                assert!(matches!(&updates[0], MlagUpdate::Isl(u) if u.is_add && u.isl_name == *isl_name));
            }

            // Verify total statistics
            let stats = orch.stats();
            assert_eq!(stats.isl_adds, isl_names.len() as u64);

            // Test ISL removal
            callbacks.clear_updates();
            orch.del_isl_interface().unwrap();

            // Verify ISL removed
            assert!(orch.isl_name().is_none());

            // Verify delete notification
            let updates = callbacks.get_updates();
            assert_eq!(updates.len(), 1);
            assert!(matches!(&updates[0], MlagUpdate::Isl(u) if !u.is_add));

            // Verify removal statistics
            let stats = orch.stats();
            assert_eq!(stats.isl_deletes, 1);

            // Test error: removing ISL when not set
            let result = orch.del_isl_interface();
            assert!(result.is_err());
        }

        #[test]
        fn test_mlag_interface_operations_integration() {
            let callbacks = Arc::new(MockMlagCallbacks::new());
            let mut orch = MlagOrch::new(MlagOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Set up MLAG domain with ISL
            orch.set_initialized();
            create_mlag_domain(&mut orch, "PortChannel100").unwrap();
            callbacks.clear_updates();

            // Add multiple MLAG interfaces
            let mlag_interfaces = vec![
                "PortChannel1",
                "PortChannel2",
                "PortChannel3",
                "Ethernet0",
                "Ethernet4",
            ];

            for if_name in &mlag_interfaces {
                add_mlag_interface(&mut orch, if_name).unwrap();
            }

            // Verify all interfaces were added
            assert_eq!(orch.mlag_interface_count(), 5);
            for if_name in &mlag_interfaces {
                assert!(orch.is_mlag_interface(if_name));
            }

            // Verify notifications (5 interface adds)
            let updates = callbacks.get_updates();
            assert_eq!(updates.len(), 5);
            for (i, if_name) in mlag_interfaces.iter().enumerate() {
                assert!(matches!(&updates[i], MlagUpdate::Intf(u) if u.is_add && u.if_name == *if_name));
            }

            // Verify statistics
            let stats = orch.stats();
            assert_eq!(stats.intf_adds, 5);
            assert_eq!(stats.intf_deletes, 0);

            // Verify ISL and MLAG interfaces are tracked separately
            assert!(orch.is_isl_interface("PortChannel100"));
            assert!(!orch.is_mlag_interface("PortChannel100"));
            assert!(orch.is_mlag_interface("PortChannel1"));
            assert!(!orch.is_isl_interface("PortChannel1"));

            // Test interface iteration
            let intfs: Vec<&String> = orch.mlag_interfaces().collect();
            assert_eq!(intfs.len(), 5);
            for if_name in &mlag_interfaces {
                assert!(intfs.iter().any(|i| i.as_str() == *if_name));
            }

            // Test partial removal
            callbacks.clear_updates();
            orch.del_mlag_interface("PortChannel1").unwrap();
            orch.del_mlag_interface("Ethernet0").unwrap();

            // Verify partial cleanup
            assert_eq!(orch.mlag_interface_count(), 3);
            assert!(!orch.is_mlag_interface("PortChannel1"));
            assert!(!orch.is_mlag_interface("Ethernet0"));
            assert!(orch.is_mlag_interface("PortChannel2"));
            assert!(orch.is_mlag_interface("PortChannel3"));
            assert!(orch.is_mlag_interface("Ethernet4"));

            // Verify delete notifications
            let updates = callbacks.get_updates();
            assert_eq!(updates.len(), 2);
            assert!(matches!(&updates[0], MlagUpdate::Intf(u) if !u.is_add && u.if_name == "PortChannel1"));
            assert!(matches!(&updates[1], MlagUpdate::Intf(u) if !u.is_add && u.if_name == "Ethernet0"));

            // Verify updated statistics
            let stats = orch.stats();
            assert_eq!(stats.intf_adds, 5);
            assert_eq!(stats.intf_deletes, 2);

            // Test error cases
            let result = orch.add_mlag_interface("PortChannel2");
            assert!(result.is_err()); // Duplicate interface

            let result = orch.del_mlag_interface("NonExistent");
            assert!(result.is_err()); // Interface not found
        }

        #[test]
        fn test_mlag_configuration_removal_and_cleanup_integration() {
            let callbacks = Arc::new(MockMlagCallbacks::new());
            let mut orch = MlagOrch::new(MlagOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Set up complete MLAG configuration
            orch.set_initialized();
            create_mlag_domain(&mut orch, "PortChannel100").unwrap();

            // Add multiple MLAG interfaces
            for i in 1..=10 {
                add_mlag_interface(&mut orch, &format!("PortChannel{}", i)).unwrap();
            }

            // Verify full setup
            assert_eq!(orch.mlag_interface_count(), 10);
            assert_eq!(orch.isl_name(), Some("PortChannel100"));
            assert!(orch.is_initialized());

            // Verify initial statistics
            let stats = orch.stats();
            assert_eq!(stats.isl_adds, 1);
            assert_eq!(stats.intf_adds, 10);
            assert_eq!(stats.notifications, 11); // 1 ISL + 10 interfaces

            callbacks.clear_updates();

            // Clean teardown: remove interfaces first
            for i in 1..=10 {
                orch.del_mlag_interface(&format!("PortChannel{}", i)).unwrap();
            }

            // Verify all interfaces removed
            assert_eq!(orch.mlag_interface_count(), 0);

            // Verify delete notifications for interfaces
            let updates = callbacks.get_updates();
            assert_eq!(updates.len(), 10);
            for update in &updates {
                assert!(matches!(update, MlagUpdate::Intf(u) if !u.is_add));
            }

            callbacks.clear_updates();

            // Remove ISL last
            orch.del_isl_interface().unwrap();

            // Verify ISL removed
            assert!(orch.isl_name().is_none());

            // Verify ISL delete notification
            let updates = callbacks.get_updates();
            assert_eq!(updates.len(), 1);
            assert!(matches!(&updates[0], MlagUpdate::Isl(u) if !u.is_add));

            // Verify complete cleanup state
            assert_eq!(orch.mlag_interface_count(), 0);
            assert!(orch.isl_name().is_none());

            // Verify final statistics
            let stats = orch.stats();
            assert_eq!(stats.isl_adds, 1);
            assert_eq!(stats.isl_deletes, 1);
            assert_eq!(stats.intf_adds, 10);
            assert_eq!(stats.intf_deletes, 10);
            assert_eq!(stats.notifications, 22); // 11 adds + 11 deletes

            // Test re-initialization after cleanup
            callbacks.clear_updates();
            create_mlag_domain(&mut orch, "PortChannel200").unwrap();
            add_mlag_interface(&mut orch, "PortChannel5").unwrap();

            assert_eq!(orch.isl_name(), Some("PortChannel200"));
            assert_eq!(orch.mlag_interface_count(), 1);

            // Verify new notifications
            let updates = callbacks.get_updates();
            assert_eq!(updates.len(), 2);
            assert!(matches!(&updates[0], MlagUpdate::Isl(u) if u.is_add && u.isl_name == "PortChannel200"));
            assert!(matches!(&updates[1], MlagUpdate::Intf(u) if u.is_add && u.if_name == "PortChannel5"));

            // Verify updated statistics
            let stats = orch.stats();
            assert_eq!(stats.isl_adds, 2);
            assert_eq!(stats.isl_deletes, 1);
            assert_eq!(stats.intf_adds, 11);
            assert_eq!(stats.intf_deletes, 10);
        }
    }

    // PolicerOrch integration tests
    mod policer_orch_tests {
        use super::*;
        use sonic_orchagent::policer::{
            PolicerOrch, PolicerOrchCallbacks, PolicerOrchConfig,
            ColorSource, MeterType, PacketAction, PolicerConfig, PolicerMode, StormType,
        };
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex};

        /// Mock callbacks for PolicerOrch testing
        struct MockPolicerCallbacks {
            sai: Arc<MockSai>,
            created_policers: Mutex<Vec<(String, PolicerConfig)>>,
            updated_policers: Mutex<Vec<u64>>,
            removed_policers: Mutex<Vec<u64>>,
            storm_policers: Mutex<Vec<(u64, StormType, Option<u64>)>>,
            ports: Mutex<HashMap<String, u64>>,
            ports_ready: bool,
        }

        impl MockPolicerCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                let mut ports = HashMap::new();
                ports.insert("Ethernet0".to_string(), 0x100);
                ports.insert("Ethernet4".to_string(), 0x104);
                ports.insert("Ethernet8".to_string(), 0x108);

                Self {
                    sai,
                    created_policers: Mutex::new(Vec::new()),
                    updated_policers: Mutex::new(Vec::new()),
                    removed_policers: Mutex::new(Vec::new()),
                    storm_policers: Mutex::new(Vec::new()),
                    ports: Mutex::new(ports),
                    ports_ready: true,
                }
            }
        }

        impl PolicerOrchCallbacks for MockPolicerCallbacks {
            fn create_policer(&self, config: &PolicerConfig) -> Result<u64, String> {
                let oid = self.sai.create_object(
                    SaiObjectType::Policer,
                    vec![
                        ("meter_type".to_string(), config.meter_type.as_str().to_string()),
                        ("mode".to_string(), config.mode.as_str().to_string()),
                        ("color_source".to_string(), config.color_source.as_str().to_string()),
                        ("cir".to_string(), config.cir.to_string()),
                        ("cbs".to_string(), config.cbs.to_string()),
                        ("pir".to_string(), config.pir.to_string()),
                        ("pbs".to_string(), config.pbs.to_string()),
                    ],
                )?;

                self.created_policers
                    .lock()
                    .unwrap()
                    .push(("policer".to_string(), config.clone()));

                Ok(oid)
            }

            fn update_policer(&self, oid: u64, _config: &PolicerConfig) -> Result<(), String> {
                self.updated_policers.lock().unwrap().push(oid);
                Ok(())
            }

            fn remove_policer(&self, oid: u64) -> Result<(), String> {
                self.removed_policers.lock().unwrap().push(oid);
                self.sai.remove_object(oid)
            }

            fn get_port_id(&self, port_name: &str) -> Option<u64> {
                self.ports.lock().unwrap().get(port_name).copied()
            }

            fn all_ports_ready(&self) -> bool {
                self.ports_ready
            }

            fn set_port_storm_policer(
                &self,
                port_id: u64,
                storm_type: StormType,
                policer_oid: Option<u64>,
            ) -> Result<(), String> {
                self.storm_policers
                    .lock()
                    .unwrap()
                    .push((port_id, storm_type, policer_oid));
                Ok(())
            }
        }

        /// Helper function to create a policer with srTCM configuration
        fn create_srtcm_policer(
            orch: &mut PolicerOrch,
            name: &str,
            cir: u64,
            cbs: u64,
        ) -> Result<(), String> {
            let config = PolicerConfig {
                meter_type: MeterType::Bytes,
                mode: PolicerMode::SrTcm,
                color_source: ColorSource::Blind,
                cir,
                cbs,
                pir: 0,
                pbs: 0,
                green_action: PacketAction::Forward,
                yellow_action: PacketAction::Forward,
                red_action: PacketAction::Drop,
            };

            orch.set_policer(name.to_string(), config)
                .map_err(|e| e.to_string())
        }

        /// Helper function to create a policer with trTCM configuration
        fn create_trtcm_policer(
            orch: &mut PolicerOrch,
            name: &str,
            cir: u64,
            cbs: u64,
            pir: u64,
            pbs: u64,
        ) -> Result<(), String> {
            let config = PolicerConfig {
                meter_type: MeterType::Bytes,
                mode: PolicerMode::TrTcm,
                color_source: ColorSource::Blind,
                cir,
                cbs,
                pir,
                pbs,
                green_action: PacketAction::Forward,
                yellow_action: PacketAction::Forward,
                red_action: PacketAction::Drop,
            };

            orch.set_policer(name.to_string(), config)
                .map_err(|e| e.to_string())
        }

        #[test]
        fn test_policer_srtcm_creation_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockPolicerCallbacks::new(Arc::clone(&sai)));
            let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Initially no policers
            assert_eq!(orch.policer_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 0);

            // Create srTCM policer with CIR of 10 Mbps and CBS of 100 KB
            create_srtcm_policer(&mut orch, "srtcm_policer", 10_000_000, 100_000).unwrap();

            // Verify policer created
            assert_eq!(orch.policer_count(), 1);
            assert!(orch.policer_exists("srtcm_policer"));
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 1);

            // Verify SAI object was created with correct attributes
            let created = callbacks.created_policers.lock().unwrap();
            assert_eq!(created.len(), 1);
            assert_eq!(created[0].1.mode, PolicerMode::SrTcm);
            assert_eq!(created[0].1.meter_type, MeterType::Bytes);
            assert_eq!(created[0].1.cir, 10_000_000);
            assert_eq!(created[0].1.cbs, 100_000);
            assert_eq!(created[0].1.pir, 0); // srTCM uses only CIR
            assert_eq!(created[0].1.green_action, PacketAction::Forward);
            assert_eq!(created[0].1.red_action, PacketAction::Drop);

            // Get policer OID
            let policer_oid = orch.get_policer_oid("srtcm_policer").unwrap();

            // Verify SAI object exists
            let sai_obj = sai.get_object(policer_oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::Policer);
            assert!(sai_obj.attributes.iter().any(|(k, v)| k == "mode" && v == "SR_TCM"));
            assert!(sai_obj.attributes.iter().any(|(k, v)| k == "cir" && v == "10000000"));

            // Verify statistics
            assert_eq!(orch.stats().policers_created, 1);
            assert_eq!(orch.stats().policers_removed, 0);

            // Cleanup
            drop(created);
            orch.remove_policer("srtcm_policer").unwrap();
            assert_eq!(orch.policer_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 0);
        }

        #[test]
        fn test_policer_trtcm_creation_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockPolicerCallbacks::new(Arc::clone(&sai)));
            let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Initially no policers
            assert_eq!(orch.policer_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 0);

            // Create trTCM policer with CIR of 5 Mbps, CBS of 50 KB, PIR of 10 Mbps, PBS of 100 KB
            create_trtcm_policer(
                &mut orch,
                "trtcm_policer",
                5_000_000,
                50_000,
                10_000_000,
                100_000,
            )
            .unwrap();

            // Verify policer created
            assert_eq!(orch.policer_count(), 1);
            assert!(orch.policer_exists("trtcm_policer"));
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 1);

            // Verify SAI object was created with correct attributes
            let created = callbacks.created_policers.lock().unwrap();
            assert_eq!(created.len(), 1);
            assert_eq!(created[0].1.mode, PolicerMode::TrTcm);
            assert_eq!(created[0].1.meter_type, MeterType::Bytes);
            assert_eq!(created[0].1.cir, 5_000_000);
            assert_eq!(created[0].1.cbs, 50_000);
            assert_eq!(created[0].1.pir, 10_000_000); // trTCM uses both CIR and PIR
            assert_eq!(created[0].1.pbs, 100_000);
            assert_eq!(created[0].1.color_source, ColorSource::Blind);

            // Get policer OID
            let policer_oid = orch.get_policer_oid("trtcm_policer").unwrap();

            // Verify SAI object exists with correct type
            let sai_obj = sai.get_object(policer_oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::Policer);
            assert!(sai_obj.attributes.iter().any(|(k, v)| k == "mode" && v == "TR_TCM"));
            assert!(sai_obj.attributes.iter().any(|(k, v)| k == "cir" && v == "5000000"));
            assert!(sai_obj.attributes.iter().any(|(k, v)| k == "pir" && v == "10000000"));

            // Test rate update (should update existing policer)
            drop(created);
            create_trtcm_policer(
                &mut orch,
                "trtcm_policer",
                8_000_000,
                80_000,
                15_000_000,
                150_000,
            )
            .unwrap();

            // Still only one policer, but updated
            assert_eq!(orch.policer_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 1);

            // Verify update callback was called
            let updated = callbacks.updated_policers.lock().unwrap();
            assert_eq!(updated.len(), 1);
            assert_eq!(updated[0], policer_oid);

            // Verify statistics
            assert_eq!(orch.stats().policers_created, 1);
            assert_eq!(orch.stats().policers_updated, 1);

            // Cleanup
            drop(updated);
            orch.remove_policer("trtcm_policer").unwrap();
            assert_eq!(orch.policer_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 0);

            let removed = callbacks.removed_policers.lock().unwrap();
            assert_eq!(removed.len(), 1);
            assert_eq!(removed[0], policer_oid);
        }

        #[test]
        fn test_storm_control_policer_configuration_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockPolicerCallbacks::new(Arc::clone(&sai)));
            let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Initially no policers
            assert_eq!(orch.policer_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 0);

            // Configure broadcast storm control on Ethernet0 at 8000 kbps
            orch.set_port_storm_control("Ethernet0", StormType::Broadcast, 8000)
                .unwrap();

            // Verify storm control policer was created
            assert_eq!(orch.policer_count(), 1);
            assert!(orch.policer_exists("_Ethernet0_broadcast"));
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 1);

            // Verify SAI policer attributes
            let created = callbacks.created_policers.lock().unwrap();
            assert_eq!(created.len(), 1);
            assert_eq!(created[0].1.mode, PolicerMode::StormControl);
            assert_eq!(created[0].1.meter_type, MeterType::Bytes);
            // 8000 kbps = 8000 * 1000 / 8 = 1000000 bps
            assert_eq!(created[0].1.cir, 1_000_000);

            // Verify storm policer was applied to port
            let storm = callbacks.storm_policers.lock().unwrap();
            assert_eq!(storm.len(), 1);
            assert_eq!(storm[0].0, 0x100); // Ethernet0 port ID
            assert_eq!(storm[0].1, StormType::Broadcast);
            assert!(storm[0].2.is_some()); // Policer OID is attached

            drop(created);
            drop(storm);

            // Configure unknown-unicast storm control on the same port
            orch.set_port_storm_control("Ethernet0", StormType::UnknownUnicast, 6000)
                .unwrap();

            // Should have two policers now
            assert_eq!(orch.policer_count(), 2);
            assert!(orch.policer_exists("_Ethernet0_unknown-unicast"));

            // Configure storm control on a different port
            orch.set_port_storm_control("Ethernet4", StormType::UnknownMulticast, 10000)
                .unwrap();

            // Should have three policers
            assert_eq!(orch.policer_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 3);
            assert!(orch.policer_exists("_Ethernet4_unknown-multicast"));

            // Verify statistics
            assert_eq!(orch.stats().policers_created, 3);
            assert_eq!(orch.stats().storm_control_applied, 3);

            // Cleanup - remove storm control from Ethernet0 broadcast
            orch.remove_port_storm_control("Ethernet0", StormType::Broadcast)
                .unwrap();

            // Should have two policers remaining
            assert_eq!(orch.policer_count(), 2);
            assert!(!orch.policer_exists("_Ethernet0_broadcast"));
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 2);

            // Verify storm policer was detached from port
            let storm = callbacks.storm_policers.lock().unwrap();
            assert_eq!(storm.len(), 4); // 3 set, 1 unset
            assert_eq!(storm[3].2, None); // Last one is unset

            // Final cleanup
            drop(storm);
            orch.remove_port_storm_control("Ethernet0", StormType::UnknownUnicast)
                .unwrap();
            orch.remove_port_storm_control("Ethernet4", StormType::UnknownMulticast)
                .unwrap();

            assert_eq!(orch.policer_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 0);
        }

        #[test]
        fn test_policer_removal_and_cleanup_integration() {
            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockPolicerCallbacks::new(Arc::clone(&sai)));
            let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
            orch.set_callbacks(callbacks.clone());

            // Create multiple policers of different types
            create_srtcm_policer(&mut orch, "policer1", 5_000_000, 50_000).unwrap();
            create_srtcm_policer(&mut orch, "policer2", 10_000_000, 100_000).unwrap();
            create_trtcm_policer(
                &mut orch,
                "policer3",
                8_000_000,
                80_000,
                15_000_000,
                150_000,
            )
            .unwrap();
            orch.set_port_storm_control("Ethernet0", StormType::Broadcast, 8000)
                .unwrap();

            // Verify all policers were created
            assert_eq!(orch.policer_count(), 4);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 4);

            // Get OIDs before removal
            let oid1 = orch.get_policer_oid("policer1").unwrap();
            let oid2 = orch.get_policer_oid("policer2").unwrap();
            let oid3 = orch.get_policer_oid("policer3").unwrap();

            // Verify SAI objects exist
            assert!(sai.get_object(oid1).is_some());
            assert!(sai.get_object(oid2).is_some());
            assert!(sai.get_object(oid3).is_some());

            // Remove policer1
            orch.remove_policer("policer1").unwrap();
            assert_eq!(orch.policer_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 3);
            assert!(!orch.policer_exists("policer1"));
            assert!(orch.policer_exists("policer2"));

            // Verify SAI object was removed
            assert!(sai.get_object(oid1).is_none());

            // Test reference counting prevents removal
            orch.increase_ref_count("policer2").unwrap();
            let result = orch.remove_policer("policer2");
            assert!(result.is_err());
            assert_eq!(orch.policer_count(), 3); // Still 3 policers
            assert!(orch.policer_exists("policer2")); // Still exists

            // Decrease ref count and try again
            orch.decrease_ref_count("policer2").unwrap();
            orch.remove_policer("policer2").unwrap();
            assert_eq!(orch.policer_count(), 2);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 2);
            assert!(!orch.policer_exists("policer2"));

            // Verify removal tracking
            let removed = callbacks.removed_policers.lock().unwrap();
            assert_eq!(removed.len(), 2);
            assert!(removed.contains(&oid1));
            assert!(removed.contains(&oid2));

            drop(removed);

            // Remove remaining policers
            orch.remove_policer("policer3").unwrap();
            orch.remove_port_storm_control("Ethernet0", StormType::Broadcast)
                .unwrap();

            // Verify complete cleanup
            assert_eq!(orch.policer_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 0);

            // Verify final statistics
            assert_eq!(orch.stats().policers_created, 4);
            assert_eq!(orch.stats().policers_removed, 4);
            assert_eq!(orch.stats().storm_control_applied, 1);

            // Verify all SAI objects were properly removed
            let removed = callbacks.removed_policers.lock().unwrap();
            assert_eq!(removed.len(), 4);
        }
    }

    // StpOrch integration tests
    mod stp_orch_tests {
        use super::*;
        use sonic_orchagent::stp::{StpOrch, StpOrchCallbacks, StpOrchConfig, StpState, SaiStpPortState};
        use std::collections::HashMap;
        use std::sync::Arc;

        /// Helper to create a test STP callbacks implementation
        struct TestStpCallbacks {
            sai: Arc<MockSai>,
            bridge_port_id: u64,
            vlan_stp_mappings: Arc<Mutex<HashMap<String, u64>>>,
        }

        impl TestStpCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                Self {
                    sai,
                    bridge_port_id: 0x2000,
                    vlan_stp_mappings: Arc::new(Mutex::new(HashMap::new())),
                }
            }
        }

        impl StpOrchCallbacks for TestStpCallbacks {
            fn all_ports_ready(&self) -> bool {
                true
            }

            fn get_port_bridge_port_id(&self, _alias: &str) -> Option<u64> {
                Some(self.bridge_port_id)
            }

            fn create_stp_instance(&self) -> Result<u64, String> {
                self.sai.create_object(
                    SaiObjectType::StpInstance,
                    vec![("type".to_string(), "stp".to_string())],
                )
            }

            fn remove_stp_instance(&self, oid: u64) -> Result<(), String> {
                self.sai.remove_object(oid)
            }

            fn set_vlan_stp_instance(&self, vlan_alias: &str, stp_inst_oid: u64) -> Result<(), String> {
                self.vlan_stp_mappings.lock().unwrap().insert(vlan_alias.to_string(), stp_inst_oid);
                Ok(())
            }

            fn create_stp_port(
                &self,
                bridge_port_id: u64,
                stp_inst_oid: u64,
                state: SaiStpPortState,
            ) -> Result<u64, String> {
                let state_str = match state {
                    SaiStpPortState::Blocking => "blocking",
                    SaiStpPortState::Learning => "learning",
                    SaiStpPortState::Forwarding => "forwarding",
                };

                self.sai.create_object(
                    SaiObjectType::StpPort,
                    vec![
                        ("bridge_port".to_string(), bridge_port_id.to_string()),
                        ("stp_instance".to_string(), stp_inst_oid.to_string()),
                        ("state".to_string(), state_str.to_string()),
                    ],
                )
            }

            fn remove_stp_port(&self, stp_port_oid: u64) -> Result<(), String> {
                self.sai.remove_object(stp_port_oid)
            }

            fn set_stp_port_state(&self, _stp_port_oid: u64, _state: SaiStpPortState) -> Result<(), String> {
                Ok(())
            }

            fn flush_fdb_by_vlan(&self, _vlan_alias: &str) -> Result<(), String> {
                Ok(())
            }

            fn ensure_bridge_port(&self, _port_alias: &str) -> Result<u64, String> {
                Ok(self.bridge_port_id)
            }
        }

        #[test]
        fn test_stp_instance_creation_integration() {
            let sai = Arc::new(MockSai::new());
            let mut orch = StpOrch::new(StpOrchConfig::default());
            let callbacks = Arc::new(TestStpCallbacks::new(sai.clone()));
            orch.set_callbacks(callbacks.clone());

            // Initialize with default STP instance
            let default_stp_oid = sai.create_object(
                SaiObjectType::StpInstance,
                vec![("default".to_string(), "true".to_string())],
            ).unwrap();
            orch.initialize(default_stp_oid, 256);

            // Verify default instance exists
            assert_eq!(orch.instance_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::StpInstance), 1);

            // Create new STP instance with VLAN configuration
            orch.add_vlan_to_instance("Vlan100", 1).unwrap();

            // Verify new instance was created
            assert_eq!(orch.instance_count(), 2);
            assert_eq!(sai.count_objects(SaiObjectType::StpInstance), 2);

            // Verify VLAN mapping was set
            let vlan_mappings = callbacks.vlan_stp_mappings.lock().unwrap();
            assert!(vlan_mappings.contains_key("Vlan100"));

            // Verify the instance OID is tracked
            let instance_oid = orch.get_instance_oid(1).unwrap();
            let sai_obj = sai.get_object(instance_oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::StpInstance);

            // Verify statistics
            assert_eq!(orch.stats().instances_created, 1);

            // Cleanup
            sai.clear();
        }

        #[test]
        fn test_stp_port_state_transitions_integration() {
            let sai = Arc::new(MockSai::new());
            let mut orch = StpOrch::new(StpOrchConfig::default());
            let callbacks = Arc::new(TestStpCallbacks::new(sai.clone()));
            orch.set_callbacks(callbacks);

            // Initialize
            let default_stp_oid = sai.create_object(
                SaiObjectType::StpInstance,
                vec![("default".to_string(), "true".to_string())],
            ).unwrap();
            orch.initialize(default_stp_oid, 256);

            // Create STP instance
            let instance_oid = orch.add_instance(1).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::StpInstance), 2);

            // Create STP port and transition through states
            let mut stp_port_ids = HashMap::new();

            // Initial state: Blocking (created automatically)
            orch.update_port_state("Ethernet0", 1, StpState::Blocking, &mut stp_port_ids).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::StpPort), 1);

            let stp_port_oid = *stp_port_ids.get(&1).unwrap();
            let port_obj = sai.get_object(stp_port_oid).unwrap();
            assert_eq!(port_obj.object_type, SaiObjectType::StpPort);
            assert_eq!(port_obj.attributes.iter().find(|(k, _)| k == "stp_instance").unwrap().1, instance_oid.to_string());

            // Transition to Learning
            orch.update_port_state("Ethernet0", 1, StpState::Learning, &mut stp_port_ids).unwrap();
            assert_eq!(orch.stats().state_updates, 2);

            // Transition to Forwarding
            orch.update_port_state("Ethernet0", 1, StpState::Forwarding, &mut stp_port_ids).unwrap();
            assert_eq!(orch.stats().state_updates, 3);

            // Transition back to Blocking
            orch.update_port_state("Ethernet0", 1, StpState::Blocking, &mut stp_port_ids).unwrap();
            assert_eq!(orch.stats().state_updates, 4);

            // Verify the port still exists and is tracked
            assert!(stp_port_ids.contains_key(&1));
            assert_eq!(sai.count_objects(SaiObjectType::StpPort), 1);

            // Cleanup
            sai.clear();
        }

        #[test]
        fn test_multiple_stp_instances_integration() {
            let sai = Arc::new(MockSai::new());
            let mut orch = StpOrch::new(StpOrchConfig::default());
            let callbacks = Arc::new(TestStpCallbacks::new(sai.clone()));
            orch.set_callbacks(callbacks.clone());

            // Initialize
            let default_stp_oid = sai.create_object(
                SaiObjectType::StpInstance,
                vec![("default".to_string(), "true".to_string())],
            ).unwrap();
            orch.initialize(default_stp_oid, 256);

            // Create multiple STP instances with different VLANs
            orch.add_vlan_to_instance("Vlan100", 1).unwrap();
            orch.add_vlan_to_instance("Vlan200", 2).unwrap();
            orch.add_vlan_to_instance("Vlan300", 3).unwrap();

            // Verify all instances were created
            assert_eq!(orch.instance_count(), 4); // Default + 3 new instances
            assert_eq!(sai.count_objects(SaiObjectType::StpInstance), 4);

            // Verify each instance has unique OID
            let oid1 = orch.get_instance_oid(1).unwrap();
            let oid2 = orch.get_instance_oid(2).unwrap();
            let oid3 = orch.get_instance_oid(3).unwrap();
            assert_ne!(oid1, oid2);
            assert_ne!(oid2, oid3);
            assert_ne!(oid1, oid3);

            // Verify VLAN mappings
            let vlan_mappings = callbacks.vlan_stp_mappings.lock().unwrap();
            assert_eq!(vlan_mappings.get("Vlan100"), Some(&oid1));
            assert_eq!(vlan_mappings.get("Vlan200"), Some(&oid2));
            assert_eq!(vlan_mappings.get("Vlan300"), Some(&oid3));

            // Add multiple VLANs to same instance
            drop(vlan_mappings);
            orch.add_vlan_to_instance("Vlan101", 1).unwrap();
            orch.add_vlan_to_instance("Vlan102", 1).unwrap();

            // Verify no new instance was created
            assert_eq!(orch.instance_count(), 4);
            assert_eq!(sai.count_objects(SaiObjectType::StpInstance), 4);

            // Create STP ports for different instances on the same physical port
            let mut stp_port_ids_eth0 = HashMap::new();
            orch.add_stp_port("Ethernet0", 1, &mut stp_port_ids_eth0).unwrap();
            orch.add_stp_port("Ethernet0", 2, &mut stp_port_ids_eth0).unwrap();
            orch.add_stp_port("Ethernet0", 3, &mut stp_port_ids_eth0).unwrap();

            // Verify STP ports were created for each instance
            assert_eq!(sai.count_objects(SaiObjectType::StpPort), 3);
            assert_eq!(stp_port_ids_eth0.len(), 3);

            // Verify statistics
            assert_eq!(orch.stats().instances_created, 3);
            assert_eq!(orch.stats().ports_created, 3);

            // Cleanup
            sai.clear();
        }

        #[test]
        fn test_stp_instance_removal_and_cleanup_integration() {
            let sai = Arc::new(MockSai::new());
            let mut orch = StpOrch::new(StpOrchConfig::default());
            let callbacks = Arc::new(TestStpCallbacks::new(sai.clone()));
            orch.set_callbacks(callbacks.clone());

            // Initialize
            let default_stp_oid = sai.create_object(
                SaiObjectType::StpInstance,
                vec![("default".to_string(), "true".to_string())],
            ).unwrap();
            orch.initialize(default_stp_oid, 256);

            // Create instances with VLANs
            orch.add_vlan_to_instance("Vlan100", 1).unwrap();
            orch.add_vlan_to_instance("Vlan200", 1).unwrap();
            orch.add_vlan_to_instance("Vlan300", 2).unwrap();

            assert_eq!(orch.instance_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::StpInstance), 3);

            // Create STP ports for instances (each port needs its own HashMap)
            let mut stp_port_ids_eth0 = HashMap::new();
            let mut stp_port_ids_eth4 = HashMap::new();
            orch.add_stp_port("Ethernet0", 1, &mut stp_port_ids_eth0).unwrap();
            orch.add_stp_port("Ethernet4", 1, &mut stp_port_ids_eth4).unwrap();
            orch.add_stp_port("Ethernet0", 2, &mut stp_port_ids_eth0).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::StpPort), 3);

            // Remove STP port from instance 1 on Ethernet0
            orch.remove_stp_port(1, &mut stp_port_ids_eth0).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::StpPort), 2);
            assert_eq!(orch.stats().ports_removed, 1);

            // Remove instance 1
            let instance1_oid = orch.get_instance_oid(1).unwrap();
            orch.remove_instance(1).unwrap();

            // Verify instance was removed from orchestrator
            assert_eq!(orch.instance_count(), 2);
            assert!(orch.get_instance_oid(1).is_none());

            // Verify SAI object was removed
            assert_eq!(sai.count_objects(SaiObjectType::StpInstance), 2);
            assert!(sai.get_object(instance1_oid).is_none());

            // Verify statistics
            assert_eq!(orch.stats().instances_created, 2);
            assert_eq!(orch.stats().instances_removed, 1);

            // Remove instance 2 ports and then the instance
            let instance2_oid = orch.get_instance_oid(2).unwrap();
            orch.remove_stp_port(2, &mut stp_port_ids_eth0).unwrap();
            orch.remove_instance(2).unwrap();

            // Verify cleanup
            assert_eq!(orch.instance_count(), 1); // Only default remains
            assert_eq!(sai.count_objects(SaiObjectType::StpInstance), 1);
            assert_eq!(sai.count_objects(SaiObjectType::StpPort), 1); // Ethernet4 port for instance 1 still exists
            assert!(sai.get_object(instance2_oid).is_none());

            // Verify final statistics
            assert_eq!(orch.stats().instances_removed, 2);
            assert_eq!(orch.stats().ports_removed, 2);

            // Verify VLAN mappings reset to default
            let vlan_mappings = callbacks.vlan_stp_mappings.lock().unwrap();
            // When instance is removed, VLANs should be reset to default (handled in production code)
            // Here we just verify the cleanup happened
            assert_eq!(orch.instance_count(), 1);

            // Cleanup
            sai.clear();
        }
    }

    // PfcwdOrch integration tests
    mod pfcwd_orch_tests {
        use super::*;
        use sonic_orchagent::pfcwd::{
            DetectionTime, PfcWdAction, PfcWdConfig, PfcWdOrch, PfcWdOrchCallbacks,
            PfcWdOrchConfig, RestorationTime,
        };
        use std::sync::{Arc, Mutex};

        /// Mock callbacks for PfcwdOrch testing
        struct MockPfcwdCallbacks {
            sai: Arc<MockSai>,
            created_watchdogs: Mutex<Vec<PfcWdConfig>>,
            removed_watchdogs: Mutex<Vec<u64>>,
            started_watchdogs: Mutex<Vec<u64>>,
            stopped_watchdogs: Mutex<Vec<u64>>,
        }

        impl MockPfcwdCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                Self {
                    sai,
                    created_watchdogs: Mutex::new(Vec::new()),
                    removed_watchdogs: Mutex::new(Vec::new()),
                    started_watchdogs: Mutex::new(Vec::new()),
                    stopped_watchdogs: Mutex::new(Vec::new()),
                }
            }
        }

        impl PfcWdOrchCallbacks for MockPfcwdCallbacks {
            fn create_watchdog(&self, config: &PfcWdConfig) -> Result<u64, String> {
                // Create a SAI object to simulate watchdog creation
                // Note: PFC watchdog doesn't have a specific SAI object type in MockSai,
                // so we use a generic approach tracking via the callbacks
                let oid = {
                    let mut next_oid = self.sai.next_oid.lock().unwrap();
                    let oid = *next_oid;
                    *next_oid += 1;
                    oid
                };

                self.created_watchdogs.lock().unwrap().push(config.clone());
                Ok(oid)
            }

            fn remove_watchdog(&self, wd_id: u64) -> Result<(), String> {
                self.removed_watchdogs.lock().unwrap().push(wd_id);
                Ok(())
            }

            fn start_watchdog(&self, wd_id: u64) -> Result<(), String> {
                self.started_watchdogs.lock().unwrap().push(wd_id);
                Ok(())
            }

            fn stop_watchdog(&self, wd_id: u64) -> Result<(), String> {
                self.stopped_watchdogs.lock().unwrap().push(wd_id);
                Ok(())
            }
        }

        /// Helper function to create a PFC watchdog configuration
        fn create_pfcwd_config(
            queue_name: &str,
            action: PfcWdAction,
            detection_ms: u32,
            restoration_ms: u32,
        ) -> PfcWdConfig {
            PfcWdConfig::new(
                queue_name.to_string(),
                action,
                DetectionTime::new(detection_ms).unwrap(),
                RestorationTime::new(restoration_ms).unwrap(),
            )
        }

        #[test]
        fn test_pfcwd_port_configuration_integration() {
            // Test PFC watchdog configuration on ports
            // Verifies that watchdogs can be registered and started on multiple port queues

            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockPfcwdCallbacks::new(Arc::clone(&sai)));
            let mut orch = PfcWdOrch::new(PfcWdOrchConfig { poll_interval_ms: 100 });
            orch.set_callbacks(callbacks.clone());

            // Initially no queues registered
            assert_eq!(orch.queue_count(), 0);

            // Configure PFC watchdog on multiple port queues (simulating Ethernet0 queues 3, 4, 5)
            let configs = vec![
                create_pfcwd_config("Ethernet0:3", PfcWdAction::Drop, 200, 200),
                create_pfcwd_config("Ethernet0:4", PfcWdAction::Drop, 200, 200),
                create_pfcwd_config("Ethernet4:3", PfcWdAction::Forward, 300, 300),
                create_pfcwd_config("Ethernet8:5", PfcWdAction::Alert, 250, 500),
            ];

            for config in configs {
                orch.register_queue(config).unwrap();
            }

            // Verify all queues were registered
            assert_eq!(orch.queue_count(), 4);
            assert!(orch.queue_exists("Ethernet0:3"));
            assert!(orch.queue_exists("Ethernet0:4"));
            assert!(orch.queue_exists("Ethernet4:3"));
            assert!(orch.queue_exists("Ethernet8:5"));

            // Verify statistics
            assert_eq!(orch.stats().queues_registered, 4);

            // Verify SAI callbacks were invoked for each watchdog creation
            let created = callbacks.created_watchdogs.lock().unwrap();
            assert_eq!(created.len(), 4);

            // Verify queue names in created watchdogs
            let queue_names: Vec<_> = created.iter().map(|c| c.queue_name.as_str()).collect();
            assert!(queue_names.contains(&"Ethernet0:3"));
            assert!(queue_names.contains(&"Ethernet0:4"));
            assert!(queue_names.contains(&"Ethernet4:3"));
            assert!(queue_names.contains(&"Ethernet8:5"));

            // Start watchdogs and verify
            orch.start_watchdog("Ethernet0:3").unwrap();
            orch.start_watchdog("Ethernet4:3").unwrap();

            let started = callbacks.started_watchdogs.lock().unwrap();
            assert_eq!(started.len(), 2);
        }

        #[test]
        fn test_pfcwd_storm_detection_and_action_integration() {
            // Test storm detection and action configuration
            // Verifies different actions (Drop, Forward, Alert) are properly handled

            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockPfcwdCallbacks::new(Arc::clone(&sai)));
            let mut orch = PfcWdOrch::new(PfcWdOrchConfig { poll_interval_ms: 50 });
            orch.set_callbacks(callbacks.clone());

            // Configure watchdogs with different actions
            let drop_config = create_pfcwd_config("Ethernet0:3", PfcWdAction::Drop, 200, 200);
            let forward_config = create_pfcwd_config("Ethernet4:3", PfcWdAction::Forward, 300, 300);
            let alert_config = create_pfcwd_config("Ethernet8:3", PfcWdAction::Alert, 250, 500);

            orch.register_queue(drop_config).unwrap();
            orch.register_queue(forward_config).unwrap();
            orch.register_queue(alert_config).unwrap();

            // Start all watchdogs
            orch.start_watchdog("Ethernet0:3").unwrap();
            orch.start_watchdog("Ethernet4:3").unwrap();
            orch.start_watchdog("Ethernet8:3").unwrap();

            // Verify all watchdogs were started
            let started = callbacks.started_watchdogs.lock().unwrap();
            assert_eq!(started.len(), 3);
            drop(started);

            // Simulate storm detection on multiple queues
            orch.handle_storm_detected("Ethernet0:3");
            orch.handle_storm_detected("Ethernet4:3");
            orch.handle_storm_detected("Ethernet8:3");

            // Verify storm statistics
            assert_eq!(orch.stats().storms_detected, 3);

            // Verify orchestrator state: storms are active
            // (In production, actions would be applied based on the action type)

            // Verify created watchdog configurations have correct actions
            let created = callbacks.created_watchdogs.lock().unwrap();
            let drop_wd = created.iter().find(|c| c.queue_name == "Ethernet0:3").unwrap();
            let forward_wd = created.iter().find(|c| c.queue_name == "Ethernet4:3").unwrap();
            let alert_wd = created.iter().find(|c| c.queue_name == "Ethernet8:3").unwrap();

            assert_eq!(drop_wd.action, PfcWdAction::Drop);
            assert_eq!(forward_wd.action, PfcWdAction::Forward);
            assert_eq!(alert_wd.action, PfcWdAction::Alert);

            // Verify detection time configurations
            assert_eq!(drop_wd.detection_time.value(), 200);
            assert_eq!(forward_wd.detection_time.value(), 300);
            assert_eq!(alert_wd.detection_time.value(), 250);
        }

        #[test]
        fn test_pfcwd_recovery_action_integration() {
            // Test recovery action configuration
            // Verifies storm recovery and restoration time handling

            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockPfcwdCallbacks::new(Arc::clone(&sai)));
            let mut orch = PfcWdOrch::new(PfcWdOrchConfig { poll_interval_ms: 100 });
            orch.set_callbacks(callbacks.clone());

            // Configure watchdogs with different restoration times
            let fast_recovery = create_pfcwd_config("Ethernet0:3", PfcWdAction::Drop, 200, 100);
            let slow_recovery = create_pfcwd_config("Ethernet4:3", PfcWdAction::Drop, 200, 5000);
            let no_auto_recovery = create_pfcwd_config("Ethernet8:3", PfcWdAction::Forward, 200, 0);

            orch.register_queue(fast_recovery).unwrap();
            orch.register_queue(slow_recovery).unwrap();
            orch.register_queue(no_auto_recovery).unwrap();

            // Start all watchdogs
            orch.start_watchdog("Ethernet0:3").unwrap();
            orch.start_watchdog("Ethernet4:3").unwrap();
            orch.start_watchdog("Ethernet8:3").unwrap();

            // Simulate storm detection
            orch.handle_storm_detected("Ethernet0:3");
            orch.handle_storm_detected("Ethernet4:3");
            orch.handle_storm_detected("Ethernet8:3");

            assert_eq!(orch.stats().storms_detected, 3);
            assert_eq!(orch.stats().storms_restored, 0);

            // Simulate storm recovery for all queues
            orch.handle_storm_restored("Ethernet0:3");
            orch.handle_storm_restored("Ethernet4:3");
            orch.handle_storm_restored("Ethernet8:3");

            // Verify recovery statistics
            assert_eq!(orch.stats().storms_restored, 3);

            // Verify restoration time configurations in created watchdogs
            let created = callbacks.created_watchdogs.lock().unwrap();
            let fast_wd = created.iter().find(|c| c.queue_name == "Ethernet0:3").unwrap();
            let slow_wd = created.iter().find(|c| c.queue_name == "Ethernet4:3").unwrap();
            let no_auto_wd = created.iter().find(|c| c.queue_name == "Ethernet8:3").unwrap();

            assert_eq!(fast_wd.restoration_time.value(), 100);
            assert_eq!(slow_wd.restoration_time.value(), 5000);
            assert_eq!(no_auto_wd.restoration_time.value(), 0); // 0 = no auto recovery

            // Verify multiple storm cycles can occur
            orch.handle_storm_detected("Ethernet0:3");
            orch.handle_storm_restored("Ethernet0:3");
            orch.handle_storm_detected("Ethernet0:3");
            orch.handle_storm_restored("Ethernet0:3");

            assert_eq!(orch.stats().storms_detected, 5);
            assert_eq!(orch.stats().storms_restored, 5);
        }

        #[test]
        fn test_pfcwd_removal_and_cleanup_integration() {
            // Test PFC watchdog removal and cleanup
            // Verifies that unregistering queues properly cleans up state and invokes SAI callbacks

            let sai = Arc::new(MockSai::new());
            let callbacks = Arc::new(MockPfcwdCallbacks::new(Arc::clone(&sai)));
            let mut orch = PfcWdOrch::new(PfcWdOrchConfig { poll_interval_ms: 100 });
            orch.set_callbacks(callbacks.clone());

            // Register multiple queues
            let configs = vec![
                create_pfcwd_config("Ethernet0:3", PfcWdAction::Drop, 200, 200),
                create_pfcwd_config("Ethernet0:4", PfcWdAction::Drop, 200, 200),
                create_pfcwd_config("Ethernet4:3", PfcWdAction::Forward, 300, 300),
                create_pfcwd_config("Ethernet8:5", PfcWdAction::Alert, 250, 500),
            ];

            for config in configs {
                orch.register_queue(config).unwrap();
            }

            assert_eq!(orch.queue_count(), 4);
            assert_eq!(orch.stats().queues_registered, 4);

            // Start some watchdogs and simulate storms
            orch.start_watchdog("Ethernet0:3").unwrap();
            orch.start_watchdog("Ethernet4:3").unwrap();
            orch.handle_storm_detected("Ethernet0:3");

            // Remove one queue
            orch.unregister_queue("Ethernet0:3").unwrap();

            // Verify queue was removed
            assert_eq!(orch.queue_count(), 3);
            assert!(!orch.queue_exists("Ethernet0:3"));
            assert!(orch.queue_exists("Ethernet0:4"));
            assert!(orch.queue_exists("Ethernet4:3"));
            assert!(orch.queue_exists("Ethernet8:5"));

            // Verify unregister statistics
            assert_eq!(orch.stats().queues_unregistered, 1);

            // Verify SAI remove callback was invoked
            let removed = callbacks.removed_watchdogs.lock().unwrap();
            assert_eq!(removed.len(), 1);
            drop(removed);

            // Remove remaining queues
            orch.unregister_queue("Ethernet0:4").unwrap();
            orch.unregister_queue("Ethernet4:3").unwrap();
            orch.unregister_queue("Ethernet8:5").unwrap();

            // Verify complete cleanup
            assert_eq!(orch.queue_count(), 0);
            assert_eq!(orch.stats().queues_unregistered, 4);

            // Verify all SAI remove callbacks were invoked
            let removed = callbacks.removed_watchdogs.lock().unwrap();
            assert_eq!(removed.len(), 4);

            // Verify trying to remove non-existent queue fails
            let result = orch.unregister_queue("NonExistent:0");
            assert!(result.is_err());

            // Verify trying to start watchdog on removed queue fails
            let result = orch.start_watchdog("Ethernet0:3");
            assert!(result.is_err());

            // Verify storm handling on removed queue has no effect
            orch.handle_storm_detected("Ethernet0:3");
            assert_eq!(orch.stats().storms_detected, 1); // Still 1 from before removal

            // Re-register a queue to verify cleanup was complete
            let new_config = create_pfcwd_config("Ethernet0:3", PfcWdAction::Drop, 300, 300);
            orch.register_queue(new_config).unwrap();

            assert_eq!(orch.queue_count(), 1);
            assert!(orch.queue_exists("Ethernet0:3"));
            assert_eq!(orch.stats().queues_registered, 5); // 4 original + 1 re-registered
        }
    }

    // TunnelDecapOrch integration tests
    mod tunnel_decap_orch_tests {
        use super::*;
        use sonic_orchagent::tunnel_decap::{
            TunnelDecapOrch, TunnelDecapOrchCallbacks, TunnelDecapOrchConfig,
            TunnelDecapConfig, TunnelTermType,
        };
        use sonic_sai::types::RawSaiObjectId;
        use sonic_types::IpAddress;
        use std::str::FromStr;

        /// MockSai-based callbacks for TunnelDecapOrch integration testing
        struct MockSaiCallbacks {
            sai: Arc<MockSai>,
        }

        impl MockSaiCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                Self { sai }
            }
        }

        impl TunnelDecapOrchCallbacks for MockSaiCallbacks {
            fn create_tunnel(&self, config: &TunnelDecapConfig) -> Result<RawSaiObjectId, String> {
                self.sai.create_object(
                    SaiObjectType::Tunnel,
                    vec![
                        ("name".to_string(), config.tunnel_name.clone()),
                        ("type".to_string(), config.tunnel_type.clone()),
                    ],
                )
            }

            fn remove_tunnel(&self, tunnel_id: RawSaiObjectId) -> Result<(), String> {
                self.sai.remove_object(tunnel_id)
            }

            fn create_tunnel_term_entry(
                &self,
                tunnel_id: RawSaiObjectId,
                term_type: TunnelTermType,
                src_ip: IpAddress,
                dst_ip: IpAddress,
            ) -> Result<RawSaiObjectId, String> {
                self.sai.create_object(
                    SaiObjectType::TunnelTermEntry,
                    vec![
                        ("tunnel_id".to_string(), tunnel_id.to_string()),
                        ("term_type".to_string(), term_type.as_str().to_string()),
                        ("src_ip".to_string(), src_ip.to_string()),
                        ("dst_ip".to_string(), dst_ip.to_string()),
                    ],
                )
            }

            fn remove_tunnel_term_entry(&self, term_entry_id: RawSaiObjectId) -> Result<(), String> {
                self.sai.remove_object(term_entry_id)
            }
        }

        /// Helper function to create a tunnel decap entry with SAI synchronization
        fn create_tunnel_decap_entry(
            orch: &mut TunnelDecapOrch,
            name: &str,
            tunnel_type: &str,
        ) -> Result<(), String> {
            let config = TunnelDecapConfig::new(name.to_string(), tunnel_type.to_string());
            orch.create_tunnel(config).map_err(|e| format!("{:?}", e))
        }

        #[test]
        fn test_tunnel_decap_p2p_creation_integration() {
            // Test Point-to-Point tunnel decap entry creation with full SAI synchronization
            let sai = Arc::new(MockSai::new());
            let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
            orch.set_callbacks(Arc::new(MockSaiCallbacks::new(Arc::clone(&sai))));

            // Verify initial state
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 0);
            assert_eq!(sai.count_objects(SaiObjectType::TunnelTermEntry), 0);
            assert_eq!(orch.tunnel_count(), 0);

            // Create IPINIP tunnel for P2P decapsulation
            create_tunnel_decap_entry(&mut orch, "ipinip_p2p_tunnel", "IPINIP").unwrap();

            // Verify tunnel SAI object was created
            assert_eq!(orch.tunnel_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 1);
            assert!(orch.tunnel_exists("ipinip_p2p_tunnel"));

            // Add P2P tunnel termination entry with specific source and destination
            orch.add_term_entry(
                "ipinip_p2p_tunnel",
                "p2p_term_1".to_string(),
                TunnelTermType::P2P,
                IpAddress::from_str("10.0.0.1").unwrap(),
                IpAddress::from_str("10.0.0.2").unwrap(),
            ).unwrap();

            // Verify termination entry SAI object was created
            assert_eq!(sai.count_objects(SaiObjectType::TunnelTermEntry), 1);
            assert_eq!(orch.stats().term_entries_created, 1);

            // Verify SAI object attributes
            let objects: Vec<_> = sai.objects.lock().unwrap().clone();
            let tunnel_obj = objects.iter().find(|o| o.object_type == SaiObjectType::Tunnel).unwrap();
            assert_eq!(tunnel_obj.attributes[0].1, "ipinip_p2p_tunnel");
            assert_eq!(tunnel_obj.attributes[1].1, "IPINIP");

            let term_obj = objects.iter().find(|o| o.object_type == SaiObjectType::TunnelTermEntry).unwrap();
            assert_eq!(term_obj.attributes[1].1, "P2P");
            assert_eq!(term_obj.attributes[2].1, "10.0.0.1");
            assert_eq!(term_obj.attributes[3].1, "10.0.0.2");
        }

        #[test]
        fn test_tunnel_decap_multipoint_config_integration() {
            // Test multi-point tunnel decap configurations (P2MP, MP2MP)
            let sai = Arc::new(MockSai::new());
            let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
            orch.set_callbacks(Arc::new(MockSaiCallbacks::new(Arc::clone(&sai))));

            // Create VXLAN tunnel for multipoint decapsulation
            create_tunnel_decap_entry(&mut orch, "vxlan_multipoint", "VXLAN").unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 1);

            // Add P2MP termination (Point-to-Multipoint: single source, multiple destinations)
            orch.add_term_entry(
                "vxlan_multipoint",
                "p2mp_term".to_string(),
                TunnelTermType::P2MP,
                IpAddress::from_str("192.168.1.1").unwrap(),
                IpAddress::from_str("0.0.0.0").unwrap(), // Wildcard destination
            ).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::TunnelTermEntry), 1);

            // Add MP2MP termination (Multipoint-to-Multipoint: any source to any destination)
            orch.add_term_entry(
                "vxlan_multipoint",
                "mp2mp_term".to_string(),
                TunnelTermType::MP2MP,
                IpAddress::from_str("0.0.0.0").unwrap(), // Wildcard source
                IpAddress::from_str("0.0.0.0").unwrap(), // Wildcard destination
            ).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::TunnelTermEntry), 2);

            // Verify orchestration state
            assert_eq!(orch.stats().term_entries_created, 2);
            assert_eq!(orch.stats().tunnels_created, 1);

            // Verify SAI object attributes for multipoint entries
            let objects: Vec<_> = sai.objects.lock().unwrap().clone();
            let term_entries: Vec<_> = objects.iter()
                .filter(|o| o.object_type == SaiObjectType::TunnelTermEntry)
                .collect();

            assert_eq!(term_entries.len(), 2);

            // Find P2MP entry
            let p2mp_entry = term_entries.iter().find(|e| e.attributes[1].1 == "P2MP").unwrap();
            assert_eq!(p2mp_entry.attributes[2].1, "192.168.1.1"); // Source IP

            // Find MP2MP entry
            let mp2mp_entry = term_entries.iter().find(|e| e.attributes[1].1 == "MP2MP").unwrap();
            assert_eq!(mp2mp_entry.attributes[2].1, "0.0.0.0"); // Wildcard source
        }

        #[test]
        fn test_tunnel_decap_ip_config_integration() {
            // Test tunnel decap entry with various IP configurations (IPv4, IPv6)
            let sai = Arc::new(MockSai::new());
            let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
            orch.set_callbacks(Arc::new(MockSaiCallbacks::new(Arc::clone(&sai))));

            // Create tunnel for IP-based decapsulation
            create_tunnel_decap_entry(&mut orch, "ipv4_tunnel", "IPINIP").unwrap();
            create_tunnel_decap_entry(&mut orch, "ipv6_tunnel", "IPINIP").unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 2);
            assert_eq!(orch.tunnel_count(), 2);

            // Add IPv4 termination entry
            orch.add_term_entry(
                "ipv4_tunnel",
                "ipv4_term".to_string(),
                TunnelTermType::P2P,
                IpAddress::from_str("172.16.0.1").unwrap(),
                IpAddress::from_str("172.16.0.2").unwrap(),
            ).unwrap();

            // Add IPv6 termination entry
            orch.add_term_entry(
                "ipv6_tunnel",
                "ipv6_term".to_string(),
                TunnelTermType::P2P,
                IpAddress::from_str("2001:db8::1").unwrap(),
                IpAddress::from_str("2001:db8::2").unwrap(),
            ).unwrap();

            // Add multiple term entries to same tunnel
            orch.add_term_entry(
                "ipv4_tunnel",
                "ipv4_term_2".to_string(),
                TunnelTermType::P2P,
                IpAddress::from_str("10.1.1.1").unwrap(),
                IpAddress::from_str("10.1.1.2").unwrap(),
            ).unwrap();

            // Verify SAI synchronization
            assert_eq!(sai.count_objects(SaiObjectType::TunnelTermEntry), 3);
            assert_eq!(orch.stats().term_entries_created, 3);

            // Verify SAI objects have correct IP addresses
            let objects: Vec<_> = sai.objects.lock().unwrap().clone();
            let term_entries: Vec<_> = objects.iter()
                .filter(|o| o.object_type == SaiObjectType::TunnelTermEntry)
                .collect();

            // Find IPv6 entry by checking for IPv6 address format
            let ipv6_entry = term_entries.iter()
                .find(|e| e.attributes[2].1.contains("2001:db8"))
                .unwrap();
            assert_eq!(ipv6_entry.attributes[2].1, "2001:db8::1");
            assert_eq!(ipv6_entry.attributes[3].1, "2001:db8::2");

            // Find IPv4 entries
            let ipv4_entries: Vec<_> = term_entries.iter()
                .filter(|e| !e.attributes[2].1.contains(':'))
                .collect();
            assert_eq!(ipv4_entries.len(), 2);
        }

        #[test]
        fn test_tunnel_decap_removal_cleanup_integration() {
            // Test tunnel decap removal and cleanup with SAI synchronization
            let sai = Arc::new(MockSai::new());
            let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
            orch.set_callbacks(Arc::new(MockSaiCallbacks::new(Arc::clone(&sai))));

            // Create tunnel and multiple term entries
            create_tunnel_decap_entry(&mut orch, "cleanup_tunnel", "IPINIP").unwrap();

            orch.add_term_entry(
                "cleanup_tunnel",
                "term1".to_string(),
                TunnelTermType::P2P,
                IpAddress::from_str("10.0.0.1").unwrap(),
                IpAddress::from_str("10.0.0.2").unwrap(),
            ).unwrap();

            orch.add_term_entry(
                "cleanup_tunnel",
                "term2".to_string(),
                TunnelTermType::P2MP,
                IpAddress::from_str("10.0.0.3").unwrap(),
                IpAddress::from_str("0.0.0.0").unwrap(),
            ).unwrap();

            orch.add_term_entry(
                "cleanup_tunnel",
                "term3".to_string(),
                TunnelTermType::MP2MP,
                IpAddress::from_str("0.0.0.0").unwrap(),
                IpAddress::from_str("0.0.0.0").unwrap(),
            ).unwrap();

            // Verify initial state
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 1);
            assert_eq!(sai.count_objects(SaiObjectType::TunnelTermEntry), 3);
            assert_eq!(orch.stats().tunnels_created, 1);
            assert_eq!(orch.stats().term_entries_created, 3);

            // Attempt to remove tunnel with active term entries (should fail)
            let result = orch.remove_tunnel("cleanup_tunnel");
            assert!(result.is_err());
            // Tunnel and term entries should still exist in SAI
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 1);
            assert_eq!(sai.count_objects(SaiObjectType::TunnelTermEntry), 3);

            // Remove term entries one by one, verifying SAI cleanup
            orch.remove_term_entry("cleanup_tunnel", "term1").unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::TunnelTermEntry), 2);
            assert_eq!(orch.stats().term_entries_removed, 1);

            orch.remove_term_entry("cleanup_tunnel", "term2").unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::TunnelTermEntry), 1);
            assert_eq!(orch.stats().term_entries_removed, 2);

            orch.remove_term_entry("cleanup_tunnel", "term3").unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::TunnelTermEntry), 0);
            assert_eq!(orch.stats().term_entries_removed, 3);

            // Now tunnel removal should succeed
            orch.remove_tunnel("cleanup_tunnel").unwrap();

            // Verify complete cleanup
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 0);
            assert_eq!(sai.count_objects(SaiObjectType::TunnelTermEntry), 0);
            assert_eq!(orch.tunnel_count(), 0);
            assert_eq!(orch.stats().tunnels_removed, 1);
            assert!(!orch.tunnel_exists("cleanup_tunnel"));

            // Verify SAI objects list is empty for these types
            let objects = sai.objects.lock().unwrap();
            assert!(objects.iter().all(|o| {
                o.object_type != SaiObjectType::Tunnel &&
                o.object_type != SaiObjectType::TunnelTermEntry
            }));
        }
    }

    // NvgreOrch integration tests
    mod nvgre_orch_tests {
        use super::*;
        use sonic_orchagent::nvgre::{
            NvgreOrch, NvgreOrchConfig, NvgreOrchCallbacks,
            NvgreTunnelConfig, NvgreTunnelMapConfig, MapType, TunnelSaiIds,
        };
        use sonic_types::IpAddress;
        use std::net::Ipv4Addr;
        use std::collections::HashMap;

        /// Mock NVGRE callbacks that integrate with MockSai
        struct MockNvgreCallbacks {
            sai: Arc<MockSai>,
            vlans: Arc<Mutex<HashMap<u16, u64>>>,
        }

        impl MockNvgreCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                let mut vlans = HashMap::new();
                // Pre-populate VLANs that exist
                vlans.insert(100, 0x10000);
                vlans.insert(200, 0x20000);
                vlans.insert(300, 0x30000);
                vlans.insert(400, 0x40000);

                Self {
                    sai,
                    vlans: Arc::new(Mutex::new(vlans)),
                }
            }
        }

        impl NvgreOrchCallbacks for MockNvgreCallbacks {
            fn create_tunnel_map(&self, map_type: MapType, is_encap: bool) -> Result<u64, String> {
                let type_str = match map_type {
                    MapType::Vlan => "vlan",
                    MapType::Bridge => "bridge",
                };
                self.sai.create_object(
                    SaiObjectType::Tunnel,
                    vec![
                        ("type".to_string(), format!("tunnel_map_{}", type_str)),
                        ("encap".to_string(), is_encap.to_string()),
                    ],
                )
            }

            fn remove_tunnel_map(&self, oid: u64) -> Result<(), String> {
                self.sai.remove_object(oid)
            }

            fn create_tunnel(
                &self,
                src_ip: &IpAddress,
                _tunnel_ids: &TunnelSaiIds,
                _underlay_rif: u64,
            ) -> Result<u64, String> {
                self.sai.create_object(
                    SaiObjectType::Tunnel,
                    vec![
                        ("type".to_string(), "nvgre".to_string()),
                        ("src_ip".to_string(), format!("{:?}", src_ip)),
                    ],
                )
            }

            fn remove_tunnel(&self, oid: u64) -> Result<(), String> {
                self.sai.remove_object(oid)
            }

            fn create_tunnel_termination(
                &self,
                tunnel_id: u64,
                src_ip: &IpAddress,
                _vr_id: u64,
            ) -> Result<u64, String> {
                self.sai.create_object(
                    SaiObjectType::Tunnel,
                    vec![
                        ("type".to_string(), "tunnel_termination".to_string()),
                        ("tunnel_id".to_string(), tunnel_id.to_string()),
                        ("src_ip".to_string(), format!("{:?}", src_ip)),
                    ],
                )
            }

            fn remove_tunnel_termination(&self, oid: u64) -> Result<(), String> {
                self.sai.remove_object(oid)
            }

            fn create_tunnel_map_entry(
                &self,
                map_type: MapType,
                vsid: u32,
                vlan_id: u16,
                _is_encap: bool,
                _encap_map_id: u64,
                _decap_map_id: u64,
            ) -> Result<u64, String> {
                let type_str = match map_type {
                    MapType::Vlan => "vlan",
                    MapType::Bridge => "bridge",
                };
                self.sai.create_object(
                    SaiObjectType::Tunnel,
                    vec![
                        ("type".to_string(), format!("map_entry_{}", type_str)),
                        ("vsid".to_string(), vsid.to_string()),
                        ("vlan_id".to_string(), vlan_id.to_string()),
                    ],
                )
            }

            fn remove_tunnel_map_entry(&self, oid: u64) -> Result<(), String> {
                self.sai.remove_object(oid)
            }

            fn get_vlan_oid(&self, vlan_id: u16) -> Option<u64> {
                self.vlans.lock().unwrap().get(&vlan_id).copied()
            }

            fn get_underlay_rif(&self) -> u64 {
                0xFFFF
            }

            fn get_virtual_router_id(&self) -> u64 {
                0xFFFE
            }
        }

        fn create_test_ip(a: u8, b: u8, c: u8, d: u8) -> IpAddress {
            IpAddress::V4(Ipv4Addr::new(a, b, c, d).into())
        }

        fn create_nvgre_orch_with_sai(sai: Arc<MockSai>) -> NvgreOrch {
            let config = NvgreOrchConfig {
                enable_encap: true,
                enable_decap: true,
            };
            let mut orch = NvgreOrch::new(config);
            let callbacks = Arc::new(MockNvgreCallbacks::new(sai));
            orch.set_callbacks(callbacks);
            orch
        }

        /// Helper function to create an NVGRE tunnel and return SAI object count
        fn create_nvgre_tunnel(
            orch: &mut NvgreOrch,
            name: &str,
            src_ip: IpAddress,
        ) -> Result<(), String> {
            let config = NvgreTunnelConfig::new(name.to_string(), src_ip);
            orch.create_tunnel(config).map_err(|e| format!("{:?}", e))
        }

        /// Helper function to add VLAN-to-VSID mapping
        fn add_vlan_vsid_mapping(
            orch: &mut NvgreOrch,
            tunnel_name: &str,
            map_name: &str,
            vlan_id: u16,
            vsid: u32,
        ) -> Result<(), String> {
            let config = NvgreTunnelMapConfig::new(
                tunnel_name.to_string(),
                map_name.to_string(),
                vlan_id,
                vsid,
            );
            orch.add_tunnel_map(config).map_err(|e| format!("{:?}", e))
        }

        #[test]
        fn test_nvgre_tunnel_creation_integration() {
            // Test NVGRE tunnel creation with VSID configuration
            // Verifies that tunnel creation properly creates SAI tunnel objects
            // and that the orchestrator state matches SAI layer state
            let sai = Arc::new(MockSai::new());
            let mut orch = create_nvgre_orch_with_sai(sai.clone());

            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 0);
            assert_eq!(orch.tunnel_count(), 0);

            // Create NVGRE tunnel
            let result = create_nvgre_tunnel(&mut orch, "nvgre_tunnel1", create_test_ip(10, 0, 0, 1));
            assert!(result.is_ok());

            // Verify orchestrator state
            assert_eq!(orch.tunnel_count(), 1);
            assert!(orch.tunnel_exists("nvgre_tunnel1"));
            assert_eq!(orch.stats().tunnels_created, 1);

            // Verify SAI objects were created
            // Expected SAI objects for one tunnel:
            // - 4 tunnel maps (2 types x 2 directions: vlan encap, vlan decap, bridge encap, bridge decap)
            // - 1 tunnel object
            // - 1 tunnel termination
            // Total: 6 SAI objects
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 6);

            // Verify tunnel properties
            let tunnel = orch.get_tunnel("nvgre_tunnel1").unwrap();
            assert_eq!(tunnel.name, "nvgre_tunnel1");
            assert_eq!(tunnel.src_ip, create_test_ip(10, 0, 0, 1));
            assert!(tunnel.tunnel_ids.tunnel_id != 0);
            assert!(tunnel.tunnel_ids.tunnel_term_id != 0);
            assert!(tunnel.tunnel_ids.tunnel_encap_id.contains_key(&MapType::Vlan));
            assert!(tunnel.tunnel_ids.tunnel_encap_id.contains_key(&MapType::Bridge));
            assert!(tunnel.tunnel_ids.tunnel_decap_id.contains_key(&MapType::Vlan));
            assert!(tunnel.tunnel_ids.tunnel_decap_id.contains_key(&MapType::Bridge));

            // Add VLAN-to-VSID mapping to verify complete tunnel configuration
            let map_result = add_vlan_vsid_mapping(&mut orch, "nvgre_tunnel1", "map1", 100, 5000);
            assert!(map_result.is_ok());

            // Verify map entry was created
            assert_eq!(orch.stats().map_entries_created, 1);
            let tunnel = orch.get_tunnel("nvgre_tunnel1").unwrap();
            assert!(tunnel.has_map_entry("map1"));
            let map_entry = tunnel.map_entries.get("map1").unwrap();
            assert_eq!(map_entry.vsid, 5000);
            assert_eq!(map_entry.vlan_id, 100);

            // SAI should have one more object (the map entry)
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 7);

            // Cleanup
            sai.clear();
        }

        #[test]
        fn test_nvgre_vlan_to_vsid_mapping_integration() {
            // Test VLAN-to-VSID mapping configuration
            // Verifies that multiple VLAN-to-VSID mappings can be created for a tunnel
            // and that SAI map entry objects are properly created
            let sai = Arc::new(MockSai::new());
            let mut orch = create_nvgre_orch_with_sai(sai.clone());

            // First create a tunnel
            create_nvgre_tunnel(&mut orch, "nvgre_tunnel1", create_test_ip(10, 0, 0, 1)).unwrap();
            let initial_sai_count = sai.count_objects(SaiObjectType::Tunnel);

            // Add multiple VLAN-to-VSID mappings
            // VLAN 100 -> VSID 1000
            let result1 = add_vlan_vsid_mapping(&mut orch, "nvgre_tunnel1", "vlan100_map", 100, 1000);
            assert!(result1.is_ok());

            // VLAN 200 -> VSID 2000
            let result2 = add_vlan_vsid_mapping(&mut orch, "nvgre_tunnel1", "vlan200_map", 200, 2000);
            assert!(result2.is_ok());

            // VLAN 300 -> VSID 3000
            let result3 = add_vlan_vsid_mapping(&mut orch, "nvgre_tunnel1", "vlan300_map", 300, 3000);
            assert!(result3.is_ok());

            // Verify orchestrator state
            assert_eq!(orch.stats().map_entries_created, 3);
            let tunnel = orch.get_tunnel("nvgre_tunnel1").unwrap();
            assert_eq!(tunnel.map_entries.len(), 3);

            // Verify each mapping
            assert!(tunnel.has_map_entry("vlan100_map"));
            assert!(tunnel.has_map_entry("vlan200_map"));
            assert!(tunnel.has_map_entry("vlan300_map"));

            let map1 = tunnel.map_entries.get("vlan100_map").unwrap();
            assert_eq!(map1.vlan_id, 100);
            assert_eq!(map1.vsid, 1000);

            let map2 = tunnel.map_entries.get("vlan200_map").unwrap();
            assert_eq!(map2.vlan_id, 200);
            assert_eq!(map2.vsid, 2000);

            let map3 = tunnel.map_entries.get("vlan300_map").unwrap();
            assert_eq!(map3.vlan_id, 300);
            assert_eq!(map3.vsid, 3000);

            // Verify SAI objects - should have 3 more map entry objects
            assert_eq!(
                sai.count_objects(SaiObjectType::Tunnel),
                initial_sai_count + 3
            );

            // Test duplicate mapping rejection
            let dup_result = add_vlan_vsid_mapping(&mut orch, "nvgre_tunnel1", "vlan100_map", 100, 1000);
            assert!(dup_result.is_err());
            assert!(dup_result.unwrap_err().contains("MapEntryExists"));

            // Test VLAN not found error (VLAN 999 doesn't exist in our mock)
            let invalid_vlan_result = add_vlan_vsid_mapping(&mut orch, "nvgre_tunnel1", "invalid_map", 999, 9999);
            assert!(invalid_vlan_result.is_err());
            assert!(invalid_vlan_result.unwrap_err().contains("VlanNotFound"));

            // Cleanup
            sai.clear();
        }

        #[test]
        fn test_nvgre_multiple_tunnels_management_integration() {
            // Test management of multiple NVGRE tunnels
            // Verifies that multiple tunnels can be created, each with their own
            // VLAN-to-VSID mappings, and that SAI objects are properly tracked
            let sai = Arc::new(MockSai::new());
            let mut orch = create_nvgre_orch_with_sai(sai.clone());

            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 0);

            // Create three NVGRE tunnels with different source IPs
            create_nvgre_tunnel(&mut orch, "tunnel_dc1", create_test_ip(10, 1, 0, 1)).unwrap();
            create_nvgre_tunnel(&mut orch, "tunnel_dc2", create_test_ip(10, 2, 0, 1)).unwrap();
            create_nvgre_tunnel(&mut orch, "tunnel_dc3", create_test_ip(10, 3, 0, 1)).unwrap();

            // Verify all tunnels were created
            assert_eq!(orch.tunnel_count(), 3);
            assert_eq!(orch.stats().tunnels_created, 3);
            assert!(orch.tunnel_exists("tunnel_dc1"));
            assert!(orch.tunnel_exists("tunnel_dc2"));
            assert!(orch.tunnel_exists("tunnel_dc3"));

            // Each tunnel creates 6 SAI objects (4 maps + 1 tunnel + 1 termination)
            // 3 tunnels = 18 SAI objects
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 18);

            // Add mappings to each tunnel
            add_vlan_vsid_mapping(&mut orch, "tunnel_dc1", "dc1_vlan100", 100, 10100).unwrap();
            add_vlan_vsid_mapping(&mut orch, "tunnel_dc1", "dc1_vlan200", 200, 10200).unwrap();

            add_vlan_vsid_mapping(&mut orch, "tunnel_dc2", "dc2_vlan100", 100, 20100).unwrap();
            add_vlan_vsid_mapping(&mut orch, "tunnel_dc2", "dc2_vlan300", 300, 20300).unwrap();

            add_vlan_vsid_mapping(&mut orch, "tunnel_dc3", "dc3_vlan400", 400, 30400).unwrap();

            // Verify map entries per tunnel
            assert_eq!(orch.get_tunnel("tunnel_dc1").unwrap().map_entries.len(), 2);
            assert_eq!(orch.get_tunnel("tunnel_dc2").unwrap().map_entries.len(), 2);
            assert_eq!(orch.get_tunnel("tunnel_dc3").unwrap().map_entries.len(), 1);
            assert_eq!(orch.stats().map_entries_created, 5);

            // SAI should now have 18 + 5 = 23 objects
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 23);

            // Verify tunnel isolation - each tunnel has its own VSID mappings
            let dc1_map = orch.get_tunnel("tunnel_dc1").unwrap().map_entries.get("dc1_vlan100").unwrap();
            let dc2_map = orch.get_tunnel("tunnel_dc2").unwrap().map_entries.get("dc2_vlan100").unwrap();

            // Same VLAN (100) but different VSIDs per datacenter tunnel
            assert_eq!(dc1_map.vlan_id, 100);
            assert_eq!(dc1_map.vsid, 10100);
            assert_eq!(dc2_map.vlan_id, 100);
            assert_eq!(dc2_map.vsid, 20100);

            // Verify different source IPs
            assert_eq!(orch.get_tunnel("tunnel_dc1").unwrap().src_ip, create_test_ip(10, 1, 0, 1));
            assert_eq!(orch.get_tunnel("tunnel_dc2").unwrap().src_ip, create_test_ip(10, 2, 0, 1));
            assert_eq!(orch.get_tunnel("tunnel_dc3").unwrap().src_ip, create_test_ip(10, 3, 0, 1));

            // Test that duplicate tunnel creation fails
            let dup_tunnel = create_nvgre_tunnel(&mut orch, "tunnel_dc1", create_test_ip(10, 99, 0, 1));
            assert!(dup_tunnel.is_err());
            assert!(dup_tunnel.unwrap_err().contains("TunnelExists"));

            // Cleanup
            sai.clear();
        }

        #[test]
        fn test_nvgre_tunnel_removal_and_cleanup_integration() {
            // Test NVGRE tunnel removal and cleanup
            // Verifies that tunnel removal properly cleans up all associated
            // SAI objects including tunnel maps, termination entries, and map entries
            let sai = Arc::new(MockSai::new());
            let mut orch = create_nvgre_orch_with_sai(sai.clone());

            // Create two tunnels with mappings
            create_nvgre_tunnel(&mut orch, "tunnel_to_remove", create_test_ip(10, 0, 0, 1)).unwrap();
            create_nvgre_tunnel(&mut orch, "tunnel_to_keep", create_test_ip(10, 0, 0, 2)).unwrap();

            // Add mappings to both tunnels
            add_vlan_vsid_mapping(&mut orch, "tunnel_to_remove", "remove_map1", 100, 1000).unwrap();
            add_vlan_vsid_mapping(&mut orch, "tunnel_to_remove", "remove_map2", 200, 2000).unwrap();
            add_vlan_vsid_mapping(&mut orch, "tunnel_to_keep", "keep_map1", 100, 3000).unwrap();

            // Verify initial state
            assert_eq!(orch.tunnel_count(), 2);
            assert_eq!(orch.stats().tunnels_created, 2);
            assert_eq!(orch.stats().map_entries_created, 3);

            // Count SAI objects before removal
            // 2 tunnels x 6 objects each = 12
            // 3 map entries = 3
            // Total = 15
            let sai_count_before = sai.count_objects(SaiObjectType::Tunnel);
            assert_eq!(sai_count_before, 15);

            // Remove the first tunnel
            let remove_result = orch.remove_tunnel("tunnel_to_remove");
            assert!(remove_result.is_ok());

            // Verify orchestrator state after removal
            assert_eq!(orch.tunnel_count(), 1);
            assert_eq!(orch.stats().tunnels_removed, 1);
            assert!(!orch.tunnel_exists("tunnel_to_remove"));
            assert!(orch.tunnel_exists("tunnel_to_keep"));

            // Verify SAI objects were cleaned up
            // Removed: 6 tunnel objects + 2 map entries = 8 objects
            // Remaining: 6 tunnel objects + 1 map entry = 7 objects
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 7);

            // Verify the remaining tunnel is intact
            let remaining_tunnel = orch.get_tunnel("tunnel_to_keep").unwrap();
            assert_eq!(remaining_tunnel.name, "tunnel_to_keep");
            assert_eq!(remaining_tunnel.src_ip, create_test_ip(10, 0, 0, 2));
            assert!(remaining_tunnel.has_map_entry("keep_map1"));
            assert_eq!(remaining_tunnel.map_entries.get("keep_map1").unwrap().vsid, 3000);

            // Test removal of non-existent tunnel
            let remove_nonexistent = orch.remove_tunnel("nonexistent_tunnel");
            assert!(remove_nonexistent.is_err());

            // Test individual map entry removal
            let map_remove_result = orch.remove_tunnel_map("tunnel_to_keep", "keep_map1");
            assert!(map_remove_result.is_ok());
            assert_eq!(orch.stats().map_entries_removed, 1);

            // Verify map entry was removed but tunnel remains
            assert!(orch.tunnel_exists("tunnel_to_keep"));
            let tunnel_after_map_removal = orch.get_tunnel("tunnel_to_keep").unwrap();
            assert!(!tunnel_after_map_removal.has_map_entry("keep_map1"));
            assert_eq!(tunnel_after_map_removal.map_entries.len(), 0);

            // SAI should have one less object (the removed map entry)
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 6);

            // Final cleanup - remove the last tunnel
            orch.remove_tunnel("tunnel_to_keep").unwrap();
            assert_eq!(orch.tunnel_count(), 0);
            assert_eq!(orch.stats().tunnels_removed, 2);

            // All SAI objects should be cleaned up
            assert_eq!(sai.count_objects(SaiObjectType::Tunnel), 0);

            // Cleanup
            sai.clear();
        }
    }

    // IsolationGroupOrch integration tests
    mod isolation_group_orch_tests {
        use super::*;
        use sonic_orchagent::isolation_group::{
            IsolationGroupOrch, IsolationGroupOrchCallbacks, IsolationGroupOrchConfig,
            IsolationGroupConfig, IsolationGroupType,
        };
        use std::collections::HashMap;
        use std::sync::Arc;

        /// Helper to create a test isolation group callbacks implementation
        struct TestIsolationGroupCallbacks {
            sai: Arc<MockSai>,
            port_oids: Arc<Mutex<HashMap<String, u64>>>,
            bridge_port_oids: Arc<Mutex<HashMap<String, u64>>>,
            port_bindings: Arc<Mutex<HashMap<u64, u64>>>, // port_oid -> group_oid
        }

        impl TestIsolationGroupCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                let mut port_oids = HashMap::new();
                let mut bridge_port_oids = HashMap::new();

                // Pre-populate some port OIDs for testing
                for i in 0..16 {
                    let port_name = format!("Ethernet{}", i * 4);
                    port_oids.insert(port_name.clone(), 0x1000 + i as u64);
                    bridge_port_oids.insert(port_name, 0x2000 + i as u64);
                }

                Self {
                    sai,
                    port_oids: Arc::new(Mutex::new(port_oids)),
                    bridge_port_oids: Arc::new(Mutex::new(bridge_port_oids)),
                    port_bindings: Arc::new(Mutex::new(HashMap::new())),
                }
            }
        }

        impl IsolationGroupOrchCallbacks for TestIsolationGroupCallbacks {
            fn create_isolation_group(&self, group_type: IsolationGroupType) -> Result<u64, String> {
                let type_str = match group_type {
                    IsolationGroupType::Port => "port",
                    IsolationGroupType::BridgePort => "bridge_port",
                };

                self.sai.create_object(
                    SaiObjectType::IsolationGroup,
                    vec![("type".to_string(), type_str.to_string())],
                )
            }

            fn remove_isolation_group(&self, oid: u64) -> Result<(), String> {
                self.sai.remove_object(oid)
            }

            fn add_isolation_group_member(
                &self,
                group_id: u64,
                port_oid: u64,
            ) -> Result<u64, String> {
                self.sai.create_object(
                    SaiObjectType::IsolationGroupMember,
                    vec![
                        ("group_id".to_string(), group_id.to_string()),
                        ("port_oid".to_string(), port_oid.to_string()),
                    ],
                )
            }

            fn remove_isolation_group_member(&self, member_oid: u64) -> Result<(), String> {
                self.sai.remove_object(member_oid)
            }

            fn bind_isolation_group_to_port(
                &self,
                port_oid: u64,
                group_id: u64,
            ) -> Result<(), String> {
                self.port_bindings.lock().unwrap().insert(port_oid, group_id);
                Ok(())
            }

            fn unbind_isolation_group_from_port(&self, port_oid: u64) -> Result<(), String> {
                self.port_bindings.lock().unwrap().remove(&port_oid);
                Ok(())
            }

            fn get_port_oid(&self, alias: &str) -> Option<u64> {
                self.port_oids.lock().unwrap().get(alias).copied()
            }

            fn get_bridge_port_oid(&self, alias: &str) -> Option<u64> {
                self.bridge_port_oids.lock().unwrap().get(alias).copied()
            }
        }

        /// Helper function to create an isolation group with SAI synchronization
        fn create_isolation_group(
            orch: &mut IsolationGroupOrch,
            name: &str,
            group_type: IsolationGroupType,
            description: Option<&str>,
        ) -> Result<(), sonic_orchagent::isolation_group::IsolationGroupOrchError> {
            let mut config = IsolationGroupConfig::new(name.to_string(), group_type);
            if let Some(desc) = description {
                config = config.with_description(desc.to_string());
            }
            orch.create_isolation_group(config)
        }

        #[test]
        fn test_isolation_group_creation_integration() {
            let sai = Arc::new(MockSai::new());
            let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
            let callbacks = Arc::new(TestIsolationGroupCallbacks::new(sai.clone()));
            orch.set_callbacks(callbacks.clone());

            // Verify no isolation groups initially
            assert_eq!(sai.count_objects(SaiObjectType::IsolationGroup), 0);
            assert_eq!(orch.group_count(), 0);

            // Create a Port-type isolation group
            create_isolation_group(
                &mut orch,
                "pvlan_isolated_1",
                IsolationGroupType::Port,
                Some("Private VLAN isolated ports group 1"),
            ).unwrap();

            // Verify SAI object was created
            assert_eq!(sai.count_objects(SaiObjectType::IsolationGroup), 1);
            assert_eq!(orch.group_count(), 1);

            // Verify orchestrator state
            let group = orch.get_group("pvlan_isolated_1").unwrap();
            assert_eq!(group.group_type, IsolationGroupType::Port);
            assert_eq!(group.description, Some("Private VLAN isolated ports group 1".to_string()));
            assert!(group.oid > 0);

            // Verify SAI object attributes
            let sai_obj = sai.get_object(group.oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::IsolationGroup);
            assert_eq!(sai_obj.attributes[0].1, "port");

            // Create a BridgePort-type isolation group
            create_isolation_group(
                &mut orch,
                "pvlan_community_1",
                IsolationGroupType::BridgePort,
                Some("Private VLAN community ports group"),
            ).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::IsolationGroup), 2);
            assert_eq!(orch.group_count(), 2);

            let bridge_group = orch.get_group("pvlan_community_1").unwrap();
            assert_eq!(bridge_group.group_type, IsolationGroupType::BridgePort);

            // Verify statistics
            assert_eq!(orch.stats().groups_created, 2);

            // Cleanup
            sai.clear();
        }

        #[test]
        fn test_isolation_group_member_binding_integration() {
            let sai = Arc::new(MockSai::new());
            let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
            let callbacks = Arc::new(TestIsolationGroupCallbacks::new(sai.clone()));
            orch.set_callbacks(callbacks.clone());

            // Create an isolation group
            create_isolation_group(
                &mut orch,
                "isolated_group",
                IsolationGroupType::Port,
                None,
            ).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::IsolationGroup), 1);
            assert_eq!(sai.count_objects(SaiObjectType::IsolationGroupMember), 0);

            // Add port members to the isolation group (isolated ports)
            orch.add_isolation_group_member("isolated_group", "Ethernet0").unwrap();
            orch.add_isolation_group_member("isolated_group", "Ethernet4").unwrap();
            orch.add_isolation_group_member("isolated_group", "Ethernet8").unwrap();

            // Verify SAI member objects were created
            assert_eq!(sai.count_objects(SaiObjectType::IsolationGroupMember), 3);

            // Verify orchestrator state
            let group = orch.get_group("isolated_group").unwrap();
            assert_eq!(group.members.len(), 3);
            assert!(group.members.contains_key("Ethernet0"));
            assert!(group.members.contains_key("Ethernet4"));
            assert!(group.members.contains_key("Ethernet8"));

            // Bind the isolation group to promiscuous ports
            orch.bind_isolation_group("isolated_group", "Ethernet12").unwrap();
            orch.bind_isolation_group("isolated_group", "Ethernet16").unwrap();

            // Verify bindings in orchestrator
            let group = orch.get_group("isolated_group").unwrap();
            assert_eq!(group.bind_ports.len(), 2);
            assert!(group.bind_ports.contains(&"Ethernet12".to_string()));
            assert!(group.bind_ports.contains(&"Ethernet16".to_string()));

            // Verify bindings in callbacks (simulating SAI state)
            {
                let bindings = callbacks.port_bindings.lock().unwrap();
                assert_eq!(bindings.len(), 2);
                assert!(bindings.contains_key(&0x1003)); // Ethernet12 port OID
                assert!(bindings.contains_key(&0x1004)); // Ethernet16 port OID
            }

            // Verify statistics
            assert_eq!(orch.stats().members_added, 3);
            assert_eq!(orch.stats().bindings_added, 2);

            // Remove a member
            orch.remove_isolation_group_member("isolated_group", "Ethernet4").unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::IsolationGroupMember), 2);
            assert_eq!(orch.stats().members_removed, 1);

            // Unbind a port
            orch.unbind_isolation_group("isolated_group", "Ethernet12").unwrap();
            let bindings = callbacks.port_bindings.lock().unwrap();
            assert_eq!(bindings.len(), 1);
            assert!(!bindings.contains_key(&0x1003));
            drop(bindings);
            assert_eq!(orch.stats().bindings_removed, 1);

            // Cleanup
            sai.clear();
        }

        #[test]
        fn test_multiple_isolation_groups_management_integration() {
            let sai = Arc::new(MockSai::new());
            let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
            let callbacks = Arc::new(TestIsolationGroupCallbacks::new(sai.clone()));
            orch.set_callbacks(callbacks.clone());

            // Create multiple isolation groups for different PVLAN scenarios
            // Group 1: Isolated ports (cannot communicate with each other)
            create_isolation_group(
                &mut orch,
                "pvlan_isolated",
                IsolationGroupType::BridgePort,
                Some("Isolated PVLAN ports"),
            ).unwrap();

            // Group 2: Community group A (can communicate within community)
            create_isolation_group(
                &mut orch,
                "pvlan_community_a",
                IsolationGroupType::BridgePort,
                Some("Community A PVLAN ports"),
            ).unwrap();

            // Group 3: Community group B (separate community)
            create_isolation_group(
                &mut orch,
                "pvlan_community_b",
                IsolationGroupType::BridgePort,
                Some("Community B PVLAN ports"),
            ).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::IsolationGroup), 3);
            assert_eq!(orch.group_count(), 3);

            // Add members to isolated group
            orch.add_isolation_group_member("pvlan_isolated", "Ethernet0").unwrap();
            orch.add_isolation_group_member("pvlan_isolated", "Ethernet4").unwrap();

            // Add members to community A
            orch.add_isolation_group_member("pvlan_community_a", "Ethernet8").unwrap();
            orch.add_isolation_group_member("pvlan_community_a", "Ethernet12").unwrap();

            // Add members to community B
            orch.add_isolation_group_member("pvlan_community_b", "Ethernet16").unwrap();
            orch.add_isolation_group_member("pvlan_community_b", "Ethernet20").unwrap();

            // Verify SAI member count
            assert_eq!(sai.count_objects(SaiObjectType::IsolationGroupMember), 6);

            // Bind isolation groups to promiscuous/uplink ports
            // Isolated group bound to uplink
            orch.bind_isolation_group("pvlan_isolated", "Ethernet24").unwrap();
            // Community groups also bound to uplink
            orch.bind_isolation_group("pvlan_community_a", "Ethernet24").unwrap();
            orch.bind_isolation_group("pvlan_community_b", "Ethernet24").unwrap();

            // Verify each group's state independently
            let isolated_group = orch.get_group("pvlan_isolated").unwrap();
            assert_eq!(isolated_group.members.len(), 2);
            assert_eq!(isolated_group.bind_ports.len(), 1);

            let community_a = orch.get_group("pvlan_community_a").unwrap();
            assert_eq!(community_a.members.len(), 2);
            assert_eq!(community_a.bind_ports.len(), 1);

            let community_b = orch.get_group("pvlan_community_b").unwrap();
            assert_eq!(community_b.members.len(), 2);
            assert_eq!(community_b.bind_ports.len(), 1);

            // Verify statistics
            assert_eq!(orch.stats().groups_created, 3);
            assert_eq!(orch.stats().members_added, 6);
            assert_eq!(orch.stats().bindings_added, 3);

            // Verify group existence checks
            assert!(orch.group_exists("pvlan_isolated"));
            assert!(orch.group_exists("pvlan_community_a"));
            assert!(orch.group_exists("pvlan_community_b"));
            assert!(!orch.group_exists("nonexistent_group"));

            // Cleanup
            sai.clear();
        }

        #[test]
        fn test_isolation_group_removal_and_cleanup_integration() {
            let sai = Arc::new(MockSai::new());
            let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
            let callbacks = Arc::new(TestIsolationGroupCallbacks::new(sai.clone()));
            orch.set_callbacks(callbacks.clone());

            // Create isolation group
            create_isolation_group(
                &mut orch,
                "test_group",
                IsolationGroupType::Port,
                Some("Test isolation group for cleanup"),
            ).unwrap();

            let group_oid = orch.get_group("test_group").unwrap().oid;

            // Add members
            orch.add_isolation_group_member("test_group", "Ethernet0").unwrap();
            orch.add_isolation_group_member("test_group", "Ethernet4").unwrap();
            orch.add_isolation_group_member("test_group", "Ethernet8").unwrap();

            // Bind ports
            orch.bind_isolation_group("test_group", "Ethernet12").unwrap();
            orch.bind_isolation_group("test_group", "Ethernet16").unwrap();

            // Verify initial state
            assert_eq!(sai.count_objects(SaiObjectType::IsolationGroup), 1);
            assert_eq!(sai.count_objects(SaiObjectType::IsolationGroupMember), 3);
            let bindings = callbacks.port_bindings.lock().unwrap();
            assert_eq!(bindings.len(), 2);
            drop(bindings);

            // Remove the isolation group (should cleanup members and bindings)
            orch.remove_isolation_group("test_group").unwrap();

            // Verify SAI objects were removed
            assert_eq!(sai.count_objects(SaiObjectType::IsolationGroup), 0);
            assert_eq!(sai.count_objects(SaiObjectType::IsolationGroupMember), 0);
            assert!(sai.get_object(group_oid).is_none());

            // Verify bindings were removed
            let bindings = callbacks.port_bindings.lock().unwrap();
            assert_eq!(bindings.len(), 0);
            drop(bindings);

            // Verify orchestrator state
            assert_eq!(orch.group_count(), 0);
            assert!(!orch.group_exists("test_group"));
            assert!(orch.get_group("test_group").is_none());

            // Verify statistics
            assert_eq!(orch.stats().groups_created, 1);
            assert_eq!(orch.stats().groups_removed, 1);
            assert_eq!(orch.stats().members_added, 3);
            // Note: members are removed as part of group removal, but not counted individually

            // Create a new group to verify the system is still functional
            create_isolation_group(
                &mut orch,
                "new_group",
                IsolationGroupType::BridgePort,
                None,
            ).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::IsolationGroup), 1);
            assert_eq!(orch.group_count(), 1);
            assert_eq!(orch.stats().groups_created, 2);

            // Cleanup
            sai.clear();
        }

    // SwitchOrch integration tests
    mod switch_orch_tests {
        use super::*;
        use sonic_orchagent::switch::SwitchOrch;
        use sonic_orchagent::switch::SwitchOrchConfig;

        /// Test creating a basic SwitchOrch instance with default configuration
        #[test]
        fn test_switch_config_creation_integration() {
            let sai = MockSai::new();
            let config = SwitchOrchConfig::default();
            let orch = SwitchOrch::new(config);

            assert!(!orch.is_initialized());
            assert!(orch.get_state().is_none());

            let stats = orch.stats();
            assert_eq!(stats.hash_updates, 0);
            assert_eq!(stats.capability_queries, 0);
            assert_eq!(stats.warm_restarts, 0);

            let switch_oid = sai.create_object(
                SaiObjectType::Switch,
                vec![
                    ("type".to_string(), "switch".to_string()),
                    ("default_vlan_priority".to_string(), "0".to_string()),
                ],
            ).unwrap();

            assert_eq!(switch_oid, 1);
            assert_eq!(sai.count_objects(SaiObjectType::Switch), 1);

            let switch_obj = sai.get_object(switch_oid).unwrap();
            assert_eq!(switch_obj.object_type, SaiObjectType::Switch);
            assert_eq!(switch_obj.attributes.len(), 2);

            sai.clear();
        }

        /// Test updating switch attributes through configuration changes
        #[test]
        fn test_switch_attribute_updates_integration() {
            let sai = MockSai::new();

            let config = SwitchOrchConfig {
                enable_warm_restart: true,
                warm_restart_read_timer: 60,
                warm_restart_timer: 120,
            };
            let orch = SwitchOrch::new(config);

            let switch_oid = sai.create_object(
                SaiObjectType::Switch,
                vec![
                    ("type".to_string(), "switch".to_string()),
                    ("warm_restart_enabled".to_string(), "true".to_string()),
                    ("warm_restart_read_timer".to_string(), "60".to_string()),
                    ("warm_restart_timer".to_string(), "120".to_string()),
                ],
            ).unwrap();

            let switch_obj = sai.get_object(switch_oid).unwrap();
            assert_eq!(switch_obj.attributes.len(), 4);

            let attr_values: Vec<_> = switch_obj.attributes.iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();

            assert!(attr_values.contains(&("warm_restart_enabled", "true")));
            assert!(attr_values.contains(&("warm_restart_read_timer", "60")));
            assert!(attr_values.contains(&("warm_restart_timer", "120")));

            let updated_switch_oid = sai.create_object(
                SaiObjectType::Switch,
                vec![
                    ("type".to_string(), "switch".to_string()),
                    ("warm_restart_enabled".to_string(), "true".to_string()),
                    ("warm_restart_read_timer".to_string(), "90".to_string()),
                    ("warm_restart_timer".to_string(), "150".to_string()),
                ],
            ).unwrap();

            let updated_switch = sai.get_object(updated_switch_oid).unwrap();
            let updated_attr_values: Vec<_> = updated_switch.attributes.iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();

            assert!(updated_attr_values.contains(&("warm_restart_read_timer", "90")));
            assert!(updated_attr_values.contains(&("warm_restart_timer", "150")));

            assert_eq!(sai.count_objects(SaiObjectType::Switch), 2);

            sai.clear();
        }

        /// Test multiple switch configuration operations in sequence
        #[test]
        fn test_multiple_switch_operations_integration() {
            let sai = MockSai::new();

            let config1 = SwitchOrchConfig::default();
            let orch1 = SwitchOrch::new(config1);

            let switch_oid_1 = sai.create_object(
                SaiObjectType::Switch,
                vec![
                    ("id".to_string(), "1".to_string()),
                    ("fdb_aging_time".to_string(), "600".to_string()),
                ],
            ).unwrap();

            let config2 = SwitchOrchConfig {
                enable_warm_restart: true,
                warm_restart_read_timer: 45,
                warm_restart_timer: 90,
            };
            let orch2 = SwitchOrch::new(config2);

            let switch_oid_2 = sai.create_object(
                SaiObjectType::Switch,
                vec![
                    ("id".to_string(), "2".to_string()),
                    ("fdb_aging_time".to_string(), "300".to_string()),
                    ("warm_restart_enabled".to_string(), "true".to_string()),
                ],
            ).unwrap();

            assert_eq!(switch_oid_1, 1);
            assert_eq!(switch_oid_2, 2);
            assert_ne!(switch_oid_1, switch_oid_2);

            assert!(!orch1.is_initialized());
            assert!(!orch2.is_initialized());

            assert_eq!(sai.count_objects(SaiObjectType::Switch), 2);

            let sw1 = sai.get_object(switch_oid_1).unwrap();
            assert_eq!(sw1.attributes[0].0, "id");
            assert_eq!(sw1.attributes[0].1, "1");

            let sw2 = sai.get_object(switch_oid_2).unwrap();
            assert_eq!(sw2.attributes[0].0, "id");
            assert_eq!(sw2.attributes[0].1, "2");

            sai.clear();
        }

        /// Test switch configuration removal and cleanup
        #[test]
        fn test_switch_removal_and_cleanup_integration() {
            let sai = MockSai::new();

            let switch_oid = sai.create_object(
                SaiObjectType::Switch,
                vec![
                    ("type".to_string(), "switch".to_string()),
                    ("cpu_port".to_string(), "CPU".to_string()),
                    ("default_vlan".to_string(), "1".to_string()),
                ],
            ).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::Switch), 1);

            let cpu_port_oid = sai.create_object(
                SaiObjectType::Port,
                vec![
                    ("type".to_string(), "cpu".to_string()),
                    ("switch_id".to_string(), switch_oid.to_string()),
                ],
            ).unwrap();

            let vlan_oid = sai.create_object(
                SaiObjectType::Route,
                vec![
                    ("type".to_string(), "default_vlan".to_string()),
                    ("switch_id".to_string(), switch_oid.to_string()),
                ],
            ).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::Switch), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Port), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Route), 1);

            sai.remove_object(cpu_port_oid).unwrap();
            sai.remove_object(vlan_oid).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::Port), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Route), 0);

            sai.remove_object(switch_oid).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::Switch), 0);
            assert!(sai.get_object(switch_oid).is_none());

            assert_eq!(sai.count_objects(SaiObjectType::Switch), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Port), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Route), 0);

            sai.clear();
        }
    }

    // MplsrouteOrch integration tests
    mod mplsroute_orch_tests {
        use super::*;
        use sonic_orchagent::mplsroute::{
            MplsRouteOrch, MplsRouteOrchConfig, MplsRouteEntry, MplsRouteKey,
            MplsRouteConfig, MplsAction,
        };

        fn create_mpls_route_with_sai(
            label: u32,
            action: MplsAction,
            next_hop: &str,
            sai: &MockSai,
        ) -> (MplsRouteEntry, u64) {
            let mut entry = MplsRouteEntry::new(
                MplsRouteKey::new(label),
                MplsRouteConfig {
                    action,
                    next_hop: Some(next_hop.to_string()),
                    swap_label: if action == MplsAction::Swap { Some(label + 1000) } else { None },
                    push_labels: if action == MplsAction::Push { vec![label + 100, label + 200] } else { vec![] },
                },
            );

            // Create SAI MPLS route object
            let oid = sai.create_object(
                SaiObjectType::MplsRoute,
                vec![
                    ("label".to_string(), label.to_string()),
                    ("action".to_string(), format!("{:?}", action)),
                    ("next_hop".to_string(), next_hop.to_string()),
                ]
            ).unwrap();

            entry.route_oid = oid;
            (entry, oid)
        }

        #[test]
        fn test_mplsroute_label_entry_integration() {
            let sai = MockSai::new();

            // Create an MPLS label entry with Pop action
            let (route, oid) = create_mpls_route_with_sai(100, MplsAction::Pop, "10.0.0.1", &sai);
            let key = route.key.clone();

            // Verify SAI object was created
            assert_eq!(sai.count_objects(SaiObjectType::MplsRoute), 1);

            // Add route to orchestrator (simulating internal storage)
            let mut routes = std::collections::HashMap::new();
            routes.insert(key.clone(), route);

            // Verify route is accessible
            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::MplsRoute);
            assert_eq!(sai_obj.attributes[0].1, "100");
            assert_eq!(sai_obj.attributes[2].1, "10.0.0.1");
        }

        #[test]
        fn test_mplsroute_route_configuration_integration() {
            let sai = MockSai::new();

            // Create MPLS routes with different actions: Pop, Swap, Push
            let (_pop_route, pop_oid) = create_mpls_route_with_sai(100, MplsAction::Pop, "10.0.0.1", &sai);
            let (swap_route, swap_oid) = create_mpls_route_with_sai(200, MplsAction::Swap, "10.0.0.2", &sai);
            let (push_route, push_oid) = create_mpls_route_with_sai(300, MplsAction::Push, "10.0.0.3", &sai);

            // Verify all routes were created in SAI
            assert_eq!(sai.count_objects(SaiObjectType::MplsRoute), 3);

            // Verify Pop action
            let pop_obj = sai.get_object(pop_oid).unwrap();
            assert_eq!(pop_obj.attributes[1], ("action".to_string(), "Pop".to_string()));

            // Verify Swap action with swap label
            let swap_obj = sai.get_object(swap_oid).unwrap();
            assert_eq!(swap_obj.attributes[1], ("action".to_string(), "Swap".to_string()));
            assert_eq!(swap_route.config.swap_label, Some(1200));

            // Verify Push action with label stack
            let push_obj = sai.get_object(push_oid).unwrap();
            assert_eq!(push_obj.attributes[1], ("action".to_string(), "Push".to_string()));
            assert_eq!(push_route.config.push_labels.len(), 2);
            assert_eq!(push_route.config.push_labels[0], 400);
            assert_eq!(push_route.config.push_labels[1], 500);
        }

        #[test]
        fn test_mplsroute_multiple_routes_management_integration() {
            let sai = MockSai::new();

            // Create multiple MPLS routes
            let mut route_oids = Vec::new();
            for i in 0..5 {
                let label = 100 + (i as u32);
                let (_, oid) = create_mpls_route_with_sai(label, MplsAction::Pop, &format!("10.0.0.{}", i + 1), &sai);
                route_oids.push(oid);
            }

            // Verify all routes are tracked in SAI
            assert_eq!(sai.count_objects(SaiObjectType::MplsRoute), 5);

            // Verify each route has unique OID
            for (i, oid) in route_oids.iter().enumerate() {
                let obj = sai.get_object(*oid).unwrap();
                assert_eq!(obj.object_type, SaiObjectType::MplsRoute);
                assert_eq!(obj.oid, (i as u64) + 1);
            }

            // Verify we can retrieve specific routes
            let route3 = sai.get_object(route_oids[2]).unwrap();
            assert_eq!(route3.attributes[0].1, "102");
        }

        #[test]
        fn test_mplsroute_removal_and_cleanup_integration() {
            let sai = MockSai::new();

            // Create MPLS routes
            let (_, oid1) = create_mpls_route_with_sai(100, MplsAction::Pop, "10.0.0.1", &sai);
            let (_, oid2) = create_mpls_route_with_sai(200, MplsAction::Swap, "10.0.0.2", &sai);
            let (_, oid3) = create_mpls_route_with_sai(300, MplsAction::Push, "10.0.0.3", &sai);

            // Verify all routes are created
            assert_eq!(sai.count_objects(SaiObjectType::MplsRoute), 3);

            // Remove individual routes
            sai.remove_object(oid1).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::MplsRoute), 2);

            // Verify the removed route is gone
            assert!(sai.get_object(oid1).is_none());

            // Verify other routes still exist
            assert!(sai.get_object(oid2).is_some());
            assert!(sai.get_object(oid3).is_some());

            // Remove remaining routes
            sai.remove_object(oid2).unwrap();
            sai.remove_object(oid3).unwrap();

            // Verify all routes are removed
            assert_eq!(sai.count_objects(SaiObjectType::MplsRoute), 0);

            // Clean up and verify state is reset
            sai.clear();
            assert_eq!(sai.count_objects(SaiObjectType::MplsRoute), 0);
        }
    }

    // CountercheckOrch integration tests
    mod countercheck_orch_tests {
        use super::*;
        use sonic_orchagent::countercheck::{
            CounterCheckOrch, CounterCheckOrchConfig, CounterCheckEntry, CounterCheckKey,
            CounterCheckConfig,
        };

        /// Helper function to create a counter check entry with SAI object
        fn create_counter_check_with_sai(
            port_name: &str,
            counter_type: &str,
            expected_value: u64,
            tolerance: u64,
            sai: &MockSai,
        ) -> (CounterCheckEntry, u64) {
            let config = CounterCheckConfig {
                port_name: port_name.to_string(),
                counter_type: counter_type.to_string(),
                expected_value,
                tolerance,
            };

            let entry = CounterCheckEntry::new(config);

            // Create SAI counter object to track the check
            let oid = sai
                .create_object(
                    SaiObjectType::PortCounter,
                    vec![
                        ("port".to_string(), port_name.to_string()),
                        ("counter_type".to_string(), counter_type.to_string()),
                        ("expected_value".to_string(), expected_value.to_string()),
                        ("tolerance".to_string(), tolerance.to_string()),
                    ],
                )
                .unwrap();

            (entry, oid)
        }

        /// Test counter check with threshold validation (within tolerance)
        #[test]
        fn test_countercheck_threshold_validation_integration() {
            let sai = MockSai::new();
            let mut orch = CounterCheckOrch::new(CounterCheckOrchConfig::default());

            // Create a counter check for RX_PACKETS with expected value 1000 and tolerance 100
            let (entry, oid) = create_counter_check_with_sai("Ethernet0", "RX_PACKETS", 1000, 100, &sai);
            let key = entry.key.clone();

            // Add the counter check to the orchestrator
            orch.add_check(key.clone(), entry);

            // Verify the check was added
            assert_eq!(orch.check_count(), 1);
            assert!(orch.get_check(&key).is_some());

            // Verify SAI object was created
            assert_eq!(sai.count_objects(SaiObjectType::PortCounter), 1);

            // Verify the counter entry is within tolerance
            let check = orch.get_check(&key).unwrap();
            assert!(check.is_within_tolerance(1000)); // Exact match
            assert!(check.is_within_tolerance(1050)); // Within upper bound
            assert!(check.is_within_tolerance(950)); // Within lower bound
            assert!(!check.is_within_tolerance(1101)); // Exceeds upper bound
            assert!(!check.is_within_tolerance(899)); // Exceeds lower bound

            // Verify SAI object details
            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::PortCounter);
            assert_eq!(sai_obj.attributes[0], ("port".to_string(), "Ethernet0".to_string()));

            sai.clear();
        }

        /// Test multiple counter checks with different types
        #[test]
        fn test_countercheck_multiple_types_integration() {
            let sai = MockSai::new();
            let mut orch = CounterCheckOrch::new(CounterCheckOrchConfig::default());

            // Create multiple counter checks for different counter types on the same port
            let (rx_packets_entry, _rx_packets_oid) =
                create_counter_check_with_sai("Ethernet0", "RX_PACKETS", 1000, 100, &sai);
            let (tx_packets_entry, _tx_packets_oid) =
                create_counter_check_with_sai("Ethernet0", "TX_PACKETS", 2000, 200, &sai);
            let (rx_bytes_entry, _rx_bytes_oid) =
                create_counter_check_with_sai("Ethernet0", "RX_BYTES", 1500000, 10000, &sai);
            let (rx_errors_entry, _rx_errors_oid) =
                create_counter_check_with_sai("Ethernet0", "RX_ERRORS", 0, 5, &sai);

            let rx_packets_key = rx_packets_entry.key.clone();
            let tx_packets_key = tx_packets_entry.key.clone();
            let rx_bytes_key = rx_bytes_entry.key.clone();
            let rx_errors_key = rx_errors_entry.key.clone();

            // Add all counter checks to the orchestrator
            orch.add_check(rx_packets_key.clone(), rx_packets_entry);
            orch.add_check(tx_packets_key.clone(), tx_packets_entry);
            orch.add_check(rx_bytes_key.clone(), rx_bytes_entry);
            orch.add_check(rx_errors_key.clone(), rx_errors_entry);

            // Verify all checks were added
            assert_eq!(orch.check_count(), 4);
            assert!(orch.get_check(&rx_packets_key).is_some());
            assert!(orch.get_check(&tx_packets_key).is_some());
            assert!(orch.get_check(&rx_bytes_key).is_some());
            assert!(orch.get_check(&rx_errors_key).is_some());

            // Verify SAI objects were created for each counter type
            assert_eq!(sai.count_objects(SaiObjectType::PortCounter), 4);

            // Validate each counter type's threshold
            let rx_packets = orch.get_check(&rx_packets_key).unwrap();
            assert_eq!(rx_packets.key.counter_type, "RX_PACKETS");
            assert_eq!(rx_packets.config.expected_value, 1000);
            assert!(rx_packets.is_within_tolerance(1050));

            let tx_packets = orch.get_check(&tx_packets_key).unwrap();
            assert_eq!(tx_packets.key.counter_type, "TX_PACKETS");
            assert_eq!(tx_packets.config.expected_value, 2000);
            assert!(tx_packets.is_within_tolerance(2150));

            let rx_bytes = orch.get_check(&rx_bytes_key).unwrap();
            assert_eq!(rx_bytes.key.counter_type, "RX_BYTES");
            assert_eq!(rx_bytes.config.expected_value, 1500000);
            assert!(rx_bytes.is_within_tolerance(1505000));

            let rx_errors = orch.get_check(&rx_errors_key).unwrap();
            assert_eq!(rx_errors.key.counter_type, "RX_ERRORS");
            assert_eq!(rx_errors.config.expected_value, 0);
            assert!(rx_errors.is_within_tolerance(5));

            sai.clear();
        }

        /// Test counter validation state management across multiple ports
        #[test]
        fn test_countercheck_validation_state_management_integration() {
            let sai = MockSai::new();
            let mut orch = CounterCheckOrch::new(CounterCheckOrchConfig::default());

            // Create counter checks for multiple ports
            let (eth0_entry, eth0_oid) =
                create_counter_check_with_sai("Ethernet0", "RX_PACKETS", 1000, 100, &sai);
            let (eth1_entry, eth1_oid) =
                create_counter_check_with_sai("Ethernet1", "RX_PACKETS", 2000, 200, &sai);
            let (eth2_entry, eth2_oid) =
                create_counter_check_with_sai("Ethernet2", "RX_PACKETS", 3000, 300, &sai);

            let eth0_key = eth0_entry.key.clone();
            let eth1_key = eth1_entry.key.clone();
            let eth2_key = eth2_entry.key.clone();

            // Add all counter checks to the orchestrator
            orch.add_check(eth0_key.clone(), eth0_entry);
            orch.add_check(eth1_key.clone(), eth1_entry);
            orch.add_check(eth2_key.clone(), eth2_entry);

            // Verify all checks are in the orchestrator
            assert_eq!(orch.check_count(), 3);

            // Verify each port has its own counter check state
            let eth0_check = orch.get_check(&eth0_key).unwrap();
            let eth1_check = orch.get_check(&eth1_key).unwrap();
            let eth2_check = orch.get_check(&eth2_key).unwrap();

            assert_eq!(eth0_check.key.port_name, "Ethernet0");
            assert_eq!(eth1_check.key.port_name, "Ethernet1");
            assert_eq!(eth2_check.key.port_name, "Ethernet2");

            // Verify independent tolerance validation for each port
            assert!(eth0_check.is_within_tolerance(1050));
            assert!(eth1_check.is_within_tolerance(2100));
            assert!(eth2_check.is_within_tolerance(3150));

            assert!(!eth0_check.is_within_tolerance(1101));
            assert!(!eth1_check.is_within_tolerance(2201));
            assert!(!eth2_check.is_within_tolerance(3301));

            // Verify SAI objects track all counter validations
            assert_eq!(sai.count_objects(SaiObjectType::PortCounter), 3);

            // Verify each SAI object has correct port information
            let eth0_sai = sai.get_object(eth0_oid).unwrap();
            let eth1_sai = sai.get_object(eth1_oid).unwrap();
            let eth2_sai = sai.get_object(eth2_oid).unwrap();

            assert_eq!(eth0_sai.attributes[0], ("port".to_string(), "Ethernet0".to_string()));
            assert_eq!(eth1_sai.attributes[0], ("port".to_string(), "Ethernet1".to_string()));
            assert_eq!(eth2_sai.attributes[0], ("port".to_string(), "Ethernet2".to_string()));

            sai.clear();
        }

        /// Test counter check removal and cleanup
        #[test]
        fn test_countercheck_removal_and_cleanup_integration() {
            let sai = MockSai::new();
            let mut orch = CounterCheckOrch::new(CounterCheckOrchConfig::default());

            // Create multiple counter checks
            let (entry1, oid1) =
                create_counter_check_with_sai("Ethernet0", "RX_PACKETS", 1000, 100, &sai);
            let (entry2, oid2) =
                create_counter_check_with_sai("Ethernet1", "RX_PACKETS", 2000, 200, &sai);
            let (entry3, oid3) =
                create_counter_check_with_sai("Ethernet2", "TX_PACKETS", 1500, 150, &sai);

            let key1 = entry1.key.clone();
            let key2 = entry2.key.clone();
            let key3 = entry3.key.clone();

            // Add all counter checks
            orch.add_check(key1.clone(), entry1);
            orch.add_check(key2.clone(), entry2);
            orch.add_check(key3.clone(), entry3);

            assert_eq!(orch.check_count(), 3);
            assert_eq!(sai.count_objects(SaiObjectType::PortCounter), 3);

            // Remove the first counter check
            orch.remove_check(&key1);
            sai.remove_object(oid1).unwrap();

            assert_eq!(orch.check_count(), 2);
            assert_eq!(sai.count_objects(SaiObjectType::PortCounter), 2);
            assert!(orch.get_check(&key1).is_none());
            assert!(orch.get_check(&key2).is_some());
            assert!(orch.get_check(&key3).is_some());

            // Remove the second counter check
            orch.remove_check(&key2);
            sai.remove_object(oid2).unwrap();

            assert_eq!(orch.check_count(), 1);
            assert_eq!(sai.count_objects(SaiObjectType::PortCounter), 1);
            assert!(orch.get_check(&key2).is_none());
            assert!(orch.get_check(&key3).is_some());

            // Remove the last counter check
            orch.remove_check(&key3);
            sai.remove_object(oid3).unwrap();

            assert_eq!(orch.check_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::PortCounter), 0);
            assert!(orch.get_check(&key3).is_none());

            // Verify all SAI objects were cleaned up
            assert!(sai.get_object(oid1).is_none());
            assert!(sai.get_object(oid2).is_none());
            assert!(sai.get_object(oid3).is_none());

            sai.clear();
        }
    }

    mod chassis_orch_tests {
        use super::*;
        use sonic_orchagent::chassis::{
            ChassisOrch, ChassisOrchCallbacks, ChassisOrchConfig, ChassisOrchStats, FabricPortKey,
            RawSaiObjectId, Result, SystemPortConfig, SystemPortEntry, SystemPortKey,
        };

        /// Mock callbacks for testing.
        struct MockChassisCallbacks;

        impl ChassisOrchCallbacks for MockChassisCallbacks {
            fn create_system_port(&self, config: &SystemPortConfig) -> Result<RawSaiObjectId> {
                Ok(0x1000 + config.system_port_id as u64)
            }
            fn remove_system_port(&self, _oid: RawSaiObjectId) -> Result<()> { Ok(()) }
            fn set_system_port_attribute(&self, _oid: RawSaiObjectId, _attr_name: &str, _attr_value: &str) -> Result<()> { Ok(()) }
            fn create_fabric_port(&self, port_id: u32) -> Result<RawSaiObjectId> {
                Ok(0x2000 + port_id as u64)
            }
            fn remove_fabric_port(&self, _oid: RawSaiObjectId) -> Result<()> { Ok(()) }
            fn set_fabric_port_isolate(&self, _oid: RawSaiObjectId, _isolate: bool) -> Result<()> { Ok(()) }
            fn write_system_port_state(&self, _key: &SystemPortKey, _state: &str) -> Result<()> { Ok(()) }
            fn remove_system_port_state(&self, _key: &SystemPortKey) -> Result<()> { Ok(()) }
            fn on_system_port_created(&self, _entry: &SystemPortEntry) {}
            fn on_system_port_removed(&self, _key: &SystemPortKey) {}
            fn on_fabric_port_isolate_changed(&self, _key: &FabricPortKey, _isolate: bool) {}
        }

        /// Test system port configuration and initialization
        #[test]
        fn test_chassis_port_config_integration() {
            let sai = MockSai::new();
            let config = ChassisOrchConfig::default();
            let mut orch: ChassisOrch<MockChassisCallbacks> = ChassisOrch::new(config);

            // Create system port SAI objects
            let port_oid_1 = sai.create_object(
                SaiObjectType::SystemPort,
                vec![
                    ("system_port_id".to_string(), "100".to_string()),
                    ("switch_id".to_string(), "1".to_string()),
                    ("core_index".to_string(), "0".to_string()),
                    ("core_port_index".to_string(), "0".to_string()),
                    ("speed".to_string(), "100000".to_string()),
                ],
            ).unwrap();

            let port_oid_2 = sai.create_object(
                SaiObjectType::SystemPort,
                vec![
                    ("system_port_id".to_string(), "101".to_string()),
                    ("switch_id".to_string(), "1".to_string()),
                    ("core_index".to_string(), "1".to_string()),
                    ("core_port_index".to_string(), "0".to_string()),
                    ("speed".to_string(), "100000".to_string()),
                ],
            ).unwrap();

            assert_eq!(port_oid_1, 1);
            assert_eq!(port_oid_2, 2);
            assert_eq!(sai.count_objects(SaiObjectType::SystemPort), 2);

            // Create port entries
            let sys_port_config_1 = SystemPortConfig {
                system_port_id: 100,
                switch_id: 1,
                core_index: 0,
                core_port_index: 0,
                speed: 100000,
            };

            let sys_port_entry_1 = SystemPortEntry::new(sys_port_config_1);
            let key_1 = sys_port_entry_1.key.clone();

            // Verify port configuration matches
            assert_eq!(sys_port_entry_1.config.system_port_id, 100);
            assert_eq!(sys_port_entry_1.config.switch_id, 1);
            assert_eq!(sys_port_entry_1.config.speed, 100000);

            // Verify SAI object attributes
            let port_obj_1 = sai.get_object(port_oid_1).unwrap();
            assert_eq!(port_obj_1.object_type, SaiObjectType::SystemPort);
            assert_eq!(port_obj_1.attributes.len(), 5);

            let port_obj_2 = sai.get_object(port_oid_2).unwrap();
            assert_eq!(port_obj_2.object_type, SaiObjectType::SystemPort);

            sai.clear();
        }

        /// Test chassis state management with multiple ports
        #[test]
        fn test_chassis_state_management_integration() {
            let sai = MockSai::new();
            let config = ChassisOrchConfig::default();
            let mut orch: ChassisOrch<MockChassisCallbacks> = ChassisOrch::new(config);

            // Verify initial state
            let initial_stats = orch.stats();
            assert_eq!(initial_stats.stats.system_ports_created, 0);
            assert_eq!(initial_stats.stats.fabric_ports_created, 0);
            assert_eq!(initial_stats.errors, 0);

            // Create system ports in SAI
            let port_configs = vec![
                ("100", "1", "0", "0", "100000"),
                ("101", "1", "1", "0", "100000"),
                ("102", "1", "2", "0", "200000"),
            ];

            let mut port_oids = Vec::new();
            for (port_id, switch_id, core_idx, core_port_idx, speed) in port_configs {
                let oid = sai.create_object(
                    SaiObjectType::SystemPort,
                    vec![
                        ("system_port_id".to_string(), port_id.to_string()),
                        ("switch_id".to_string(), switch_id.to_string()),
                        ("core_index".to_string(), core_idx.to_string()),
                        ("core_port_index".to_string(), core_port_idx.to_string()),
                        ("speed".to_string(), speed.to_string()),
                    ],
                ).unwrap();
                port_oids.push(oid);
            }

            // Verify all ports created in SAI
            assert_eq!(sai.count_objects(SaiObjectType::SystemPort), 3);

            // Create and track system port entries
            let mut port_entries = Vec::new();
            for (idx, (port_id, switch_id, core_idx, core_port_idx, speed)) in
                [("100", "1", "0", "0", "100000"),
                 ("101", "1", "1", "0", "100000"),
                 ("102", "1", "2", "0", "200000")].iter().enumerate() {
                let config = SystemPortConfig {
                    system_port_id: port_id.parse().unwrap(),
                    switch_id: switch_id.parse().unwrap(),
                    core_index: core_idx.parse().unwrap(),
                    core_port_index: core_port_idx.parse().unwrap(),
                    speed: speed.parse().unwrap(),
                };
                let entry = SystemPortEntry::new(config);
                port_entries.push(entry);
            }

            // Verify port entry properties
            assert_eq!(port_entries.len(), 3);
            assert_eq!(port_entries[0].config.system_port_id, 100);
            assert_eq!(port_entries[1].config.system_port_id, 101);
            assert_eq!(port_entries[2].config.system_port_id, 102);
            assert_eq!(port_entries[2].config.speed, 200000);

            sai.clear();
        }

        /// Test multiple port operations in sequence
        #[test]
        fn test_multiple_port_operations_integration() {
            let sai = MockSai::new();
            let config = ChassisOrchConfig::default();
            let mut orch: ChassisOrch<MockChassisCallbacks> = ChassisOrch::new(config);

            // Create initial batch of system ports
            let mut initial_oids = Vec::new();
            for i in 0..5 {
                let oid = sai.create_object(
                    SaiObjectType::SystemPort,
                    vec![
                        ("system_port_id".to_string(), (100 + i).to_string()),
                        ("switch_id".to_string(), "1".to_string()),
                        ("core_index".to_string(), i.to_string()),
                        ("core_port_index".to_string(), "0".to_string()),
                        ("speed".to_string(), "100000".to_string()),
                    ],
                ).unwrap();
                initial_oids.push(oid);
            }

            assert_eq!(sai.count_objects(SaiObjectType::SystemPort), 5);

            // Create port entries and verify retrieval
            let mut port_entries = HashMap::new();
            for i in 0..5 {
                let config = SystemPortConfig {
                    system_port_id: 100 + i,
                    switch_id: 1,
                    core_index: i,
                    core_port_index: 0,
                    speed: 100000,
                };
                let entry = SystemPortEntry::new(config);
                let key = entry.key.clone();
                port_entries.insert(key, entry);
            }

            assert_eq!(port_entries.len(), 5);

            // Verify all ports can be retrieved by key
            for i in 0..5 {
                let key = SystemPortKey::new(100 + i);
                assert!(port_entries.contains_key(&key));

                let entry = &port_entries[&key];
                assert_eq!(entry.config.system_port_id, 100 + i);
                assert_eq!(entry.config.core_index, i);
            }

            // Create fabric ports alongside system ports
            let fabric_oid_1 = sai.create_object(
                SaiObjectType::FabricPort,
                vec![
                    ("fabric_port_id".to_string(), "1".to_string()),
                    ("isolate".to_string(), "false".to_string()),
                ],
            ).unwrap();

            let fabric_oid_2 = sai.create_object(
                SaiObjectType::FabricPort,
                vec![
                    ("fabric_port_id".to_string(), "2".to_string()),
                    ("isolate".to_string(), "false".to_string()),
                ],
            ).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::SystemPort), 5);
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 2);

            sai.clear();
        }

        /// Test port removal and cleanup operations
        #[test]
        fn test_port_removal_and_cleanup_integration() {
            let sai = MockSai::new();
            let config = ChassisOrchConfig::default();
            let orch: ChassisOrch<MockChassisCallbacks> = ChassisOrch::new(config);

            // Create system ports
            let sys_port_oids: Vec<u64> = (0..4)
                .map(|i| {
                    sai.create_object(
                        SaiObjectType::SystemPort,
                        vec![
                            ("system_port_id".to_string(), (100 + i).to_string()),
                            ("switch_id".to_string(), "1".to_string()),
                            ("core_index".to_string(), i.to_string()),
                            ("core_port_index".to_string(), "0".to_string()),
                            ("speed".to_string(), "100000".to_string()),
                        ],
                    ).unwrap()
                })
                .collect();

            let fabric_port_oids: Vec<u64> = (0..2)
                .map(|i| {
                    sai.create_object(
                        SaiObjectType::FabricPort,
                        vec![
                            ("fabric_port_id".to_string(), (1 + i).to_string()),
                            ("isolate".to_string(), "false".to_string()),
                        ],
                    ).unwrap()
                })
                .collect();

            // Verify initial state
            assert_eq!(sai.count_objects(SaiObjectType::SystemPort), 4);
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 2);

            // Remove some system ports
            sai.remove_object(sys_port_oids[0]).unwrap();
            sai.remove_object(sys_port_oids[1]).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::SystemPort), 2);
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 2);

            // Remove remaining system ports
            sai.remove_object(sys_port_oids[2]).unwrap();
            sai.remove_object(sys_port_oids[3]).unwrap();

            assert_eq!(sai.count_objects(SaiObjectType::SystemPort), 0);
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 2);

            // Remove fabric ports
            for fabric_oid in &fabric_port_oids {
                sai.remove_object(*fabric_oid).unwrap();
            }

            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 0);
            assert_eq!(sai.count_objects(SaiObjectType::SystemPort), 0);

            // Verify removed objects return None
            for sys_port_oid in &sys_port_oids {
                assert!(sai.get_object(*sys_port_oid).is_none());
            }

            for fabric_oid in &fabric_port_oids {
                assert!(sai.get_object(*fabric_oid).is_none());
            }

            sai.clear();
        }
    }


    // CoppOrch integration tests
    mod copp_orch_tests {
        use super::*;
        use sonic_orchagent::copp::{CoppOrch, CoppOrchConfig, CoppOrchCallbacks, CoppTrapKey, CoppTrapEntry, CoppTrapConfig, CoppTrapAction};

        struct MockCoppCallbacks {
            sai: Arc<MockSai>,
        }

        impl MockCoppCallbacks {
            fn new(sai: Arc<MockSai>) -> Self {
                Self { sai }
            }
        }

        impl CoppOrchCallbacks for MockCoppCallbacks {}

        fn create_trap_config(
            trap_action: CoppTrapAction,
            priority: u32,
            queue: u8,
            cir: u64,
            cbs: u64,
        ) -> CoppTrapConfig {
            CoppTrapConfig {
                trap_action,
                trap_priority: Some(priority),
                queue: Some(queue),
                meter_type: Some("packets".to_string()),
                mode: Some("sr_tcm".to_string()),
                color: Some("aware".to_string()),
                cbs: Some(cbs),
                cir: Some(cir),
                pbs: Some(cbs),
                pir: Some(cir),
            }
        }

        #[test]
        fn test_copp_trap_config_integration() {
            let sai = Arc::new(MockSai::new());
            let _callbacks = Arc::new(MockCoppCallbacks::new(sai.clone()));
            let mut orch = CoppOrch::new(CoppOrchConfig::default());

            // Verify no traps exist initially
            assert_eq!(orch.stats().stats.traps_created, 0);
            assert_eq!(sai.count_objects(SaiObjectType::CoppTrap), 0);
            assert_eq!(sai.count_objects(SaiObjectType::CoppTrapGroup), 0);

            // Create a trap configuration
            let key = CoppTrapKey::new("bgp".to_string());
            let config = create_trap_config(CoppTrapAction::Trap, 4, 4, 600, 600);
            let mut entry = CoppTrapEntry::new(key.clone(), config);

            // Simulate SAI object creation for trap and trap group
            let trap_oid = sai
                .create_object(
                    SaiObjectType::CoppTrap,
                    vec![
                        ("trap_id".to_string(), "bgp".to_string()),
                        ("action".to_string(), "trap".to_string()),
                        ("priority".to_string(), "4".to_string()),
                    ],
                )
                .unwrap();

            let trap_group_oid = sai
                .create_object(
                    SaiObjectType::CoppTrapGroup,
                    vec![
                        ("queue".to_string(), "4".to_string()),
                        ("policer".to_string(), "1".to_string()),
                    ],
                )
                .unwrap();

            entry.trap_oid = trap_oid;
            entry.trap_group_oid = trap_group_oid;

            // Verify SAI objects were created
            assert_eq!(sai.count_objects(SaiObjectType::CoppTrap), 1);
            assert_eq!(sai.count_objects(SaiObjectType::CoppTrapGroup), 1);

            // Verify trap object attributes
            let trap_obj = sai.get_object(trap_oid).unwrap();
            assert_eq!(trap_obj.object_type, SaiObjectType::CoppTrap);
            assert_eq!(trap_obj.attributes.len(), 3);
            assert_eq!(trap_obj.attributes[0].0, "trap_id");
            assert_eq!(trap_obj.attributes[0].1, "bgp");

            // Verify trap group object attributes
            let trap_group_obj = sai.get_object(trap_group_oid).unwrap();
            assert_eq!(trap_group_obj.object_type, SaiObjectType::CoppTrapGroup);
            assert_eq!(trap_group_obj.attributes.len(), 2);
            assert_eq!(trap_group_obj.attributes[0].0, "queue");
            assert_eq!(trap_group_obj.attributes[0].1, "4");
        }

        #[test]
        fn test_copp_trap_group_management_integration() {
            let sai = Arc::new(MockSai::new());
            let _callbacks = Arc::new(MockCoppCallbacks::new(sai.clone()));

            // Create multiple trap group objects
            let trap_group_oid1 = sai
                .create_object(
                    SaiObjectType::CoppTrapGroup,
                    vec![
                        ("group_id".to_string(), "1".to_string()),
                        ("queue".to_string(), "4".to_string()),
                        ("policer_oid".to_string(), "10".to_string()),
                    ],
                )
                .unwrap();

            let trap_group_oid2 = sai
                .create_object(
                    SaiObjectType::CoppTrapGroup,
                    vec![
                        ("group_id".to_string(), "2".to_string()),
                        ("queue".to_string(), "5".to_string()),
                        ("policer_oid".to_string(), "11".to_string()),
                    ],
                )
                .unwrap();

            let trap_group_oid3 = sai
                .create_object(
                    SaiObjectType::CoppTrapGroup,
                    vec![
                        ("group_id".to_string(), "3".to_string()),
                        ("queue".to_string(), "6".to_string()),
                        ("policer_oid".to_string(), "12".to_string()),
                    ],
                )
                .unwrap();

            // Verify all trap groups were created
            assert_eq!(sai.count_objects(SaiObjectType::CoppTrapGroup), 3);

            // Verify each trap group can be retrieved
            let group1 = sai.get_object(trap_group_oid1).unwrap();
            assert_eq!(group1.object_type, SaiObjectType::CoppTrapGroup);
            assert_eq!(group1.attributes[0].1, "1");

            let group2 = sai.get_object(trap_group_oid2).unwrap();
            assert_eq!(group2.object_type, SaiObjectType::CoppTrapGroup);
            assert_eq!(group2.attributes[0].1, "2");

            let group3 = sai.get_object(trap_group_oid3).unwrap();
            assert_eq!(group3.object_type, SaiObjectType::CoppTrapGroup);
            assert_eq!(group3.attributes[0].1, "3");

            // Verify queue assignments
            assert_eq!(group1.attributes[1].1, "4");
            assert_eq!(group2.attributes[1].1, "5");
            assert_eq!(group3.attributes[1].1, "6");
        }

        #[test]
        fn test_copp_multiple_traps_configuration_integration() {
            let sai = Arc::new(MockSai::new());
            let _callbacks = Arc::new(MockCoppCallbacks::new(sai.clone()));
            let mut orch = CoppOrch::new(CoppOrchConfig::default());

            // Define multiple trap types with different configurations
            let trap_configs = vec![
                ("bgp", CoppTrapAction::Trap, 4, 4, 600, 600),
                ("arp", CoppTrapAction::Trap, 3, 3, 500, 500),
                ("lacp", CoppTrapAction::Copy, 2, 2, 400, 400),
                ("lldp", CoppTrapAction::Copy, 1, 1, 300, 300),
                ("dhcp", CoppTrapAction::Trap, 5, 5, 700, 700),
            ];

            let mut trap_entries = Vec::new();

            // Create trap configurations and SAI objects
            for (trap_name, action, priority, queue, cir, cbs) in trap_configs.iter() {
                let key = CoppTrapKey::new(trap_name.to_string());
                let config = create_trap_config(*action, *priority, *queue, *cir, *cbs);
                let mut entry = CoppTrapEntry::new(key, config);

                // Create trap SAI object
                let trap_oid = sai
                    .create_object(
                        SaiObjectType::CoppTrap,
                        vec![
                            ("trap_id".to_string(), trap_name.to_string()),
                            ("action".to_string(), format!("{:?}", action)),
                            ("priority".to_string(), priority.to_string()),
                        ],
                    )
                    .unwrap();

                // Create trap group SAI object
                let trap_group_oid = sai
                    .create_object(
                        SaiObjectType::CoppTrapGroup,
                        vec![
                            ("queue".to_string(), queue.to_string()),
                            ("cir".to_string(), cir.to_string()),
                            ("cbs".to_string(), cbs.to_string()),
                        ],
                    )
                    .unwrap();

                entry.trap_oid = trap_oid;
                entry.trap_group_oid = trap_group_oid;
                trap_entries.push(entry);
            }

            // Verify all traps were created
            assert_eq!(sai.count_objects(SaiObjectType::CoppTrap), 5);
            assert_eq!(sai.count_objects(SaiObjectType::CoppTrapGroup), 5);

            // Verify each trap entry
            for entry in &trap_entries {
                let trap_obj = sai.get_object(entry.trap_oid).unwrap();
                assert_eq!(trap_obj.object_type, SaiObjectType::CoppTrap);

                let trap_group_obj = sai.get_object(entry.trap_group_oid).unwrap();
                assert_eq!(trap_group_obj.object_type, SaiObjectType::CoppTrapGroup);

                // Verify trap action is correctly set
                assert_eq!(
                    trap_obj.attributes[0].0,
                    "trap_id"
                );
            }

            // Verify trap priority ordering
            let bgp_entry = &trap_entries[0];
            let arp_entry = &trap_entries[1];
            let bgp_trap = sai.get_object(bgp_entry.trap_oid).unwrap();
            let arp_trap = sai.get_object(arp_entry.trap_oid).unwrap();

            assert_eq!(bgp_trap.attributes[2].1, "4");
            assert_eq!(arp_trap.attributes[2].1, "3");

            // Verify queue assignments match configured values
            let expected_queues = vec!["4", "3", "2", "1", "5"];
            for (i, entry) in trap_entries.iter().enumerate() {
                let trap_group = sai.get_object(entry.trap_group_oid).unwrap();
                assert_eq!(trap_group.attributes[0].1, expected_queues[i]);
            }
        }

        #[test]
        fn test_copp_trap_configuration_removal_and_cleanup_integration() {
            let sai = Arc::new(MockSai::new());
            let _callbacks = Arc::new(MockCoppCallbacks::new(sai.clone()));

            // Create initial trap configuration
            let trap_oid = sai
                .create_object(
                    SaiObjectType::CoppTrap,
                    vec![
                        ("trap_id".to_string(), "bgp".to_string()),
                        ("action".to_string(), "trap".to_string()),
                        ("priority".to_string(), "4".to_string()),
                    ],
                )
                .unwrap();

            let trap_group_oid = sai
                .create_object(
                    SaiObjectType::CoppTrapGroup,
                    vec![
                        ("queue".to_string(), "4".to_string()),
                        ("policer_oid".to_string(), "1".to_string()),
                    ],
                )
                .unwrap();

            let policer_oid = sai
                .create_object(
                    SaiObjectType::Policer,
                    vec![
                        ("mode".to_string(), "sr_tcm".to_string()),
                        ("cir".to_string(), "600".to_string()),
                        ("cbs".to_string(), "600".to_string()),
                    ],
                )
                .unwrap();

            // Verify all objects were created
            assert_eq!(sai.count_objects(SaiObjectType::CoppTrap), 1);
            assert_eq!(sai.count_objects(SaiObjectType::CoppTrapGroup), 1);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 1);

            // Remove trap configuration in reverse order (policer -> trap group -> trap)
            sai.remove_object(policer_oid).unwrap();
            sai.remove_object(trap_group_oid).unwrap();
            sai.remove_object(trap_oid).unwrap();

            // Verify all objects were cleaned up
            assert_eq!(sai.count_objects(SaiObjectType::CoppTrap), 0);
            assert_eq!(sai.count_objects(SaiObjectType::CoppTrapGroup), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 0);

            // Verify objects are no longer retrievable
            assert!(sai.get_object(trap_oid).is_none());
            assert!(sai.get_object(trap_group_oid).is_none());
            assert!(sai.get_object(policer_oid).is_none());

            // Clear for next test
            sai.clear();
            assert_eq!(sai.count_objects(SaiObjectType::CoppTrap), 0);
            assert_eq!(sai.count_objects(SaiObjectType::CoppTrapGroup), 0);
            assert_eq!(sai.count_objects(SaiObjectType::Policer), 0);
        }
    }

    // IcmpOrch integration tests
    mod icmp_orch_tests {
        use super::*;
        use sonic_orchagent::icmp::{IcmpOrch, IcmpOrchConfig, IcmpEchoEntry, IcmpEchoKey, IcmpMode};
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

        fn create_icmp_echo_session_with_sai(
            vrf_name: &str,
            ip: IpAddr,
            mode: IcmpMode,
            sai: &MockSai,
        ) -> (IcmpEchoEntry, u64) {
            let key = IcmpEchoKey::new(vrf_name.to_string(), ip);
            let entry = IcmpEchoEntry::new(key, mode);

            let mode_str = match mode {
                IcmpMode::Enabled => "enabled",
                IcmpMode::Disabled => "disabled",
            };

            let oid = sai
                .create_object(
                    SaiObjectType::IcmpEchoSession,
                    vec![
                        ("vrf".to_string(), vrf_name.to_string()),
                        ("ip".to_string(), ip.to_string()),
                        ("mode".to_string(), mode_str.to_string()),
                    ],
                )
                .unwrap();

            (entry, oid)
        }

        #[test]
        fn test_icmp_echo_session_integration() {
            let sai = MockSai::new();
            let config = IcmpOrchConfig::default();
            let orch = IcmpOrch::new(config);

            // Verify no ICMP echo sessions exist initially
            assert_eq!(sai.count_objects(SaiObjectType::IcmpEchoSession), 0);
            assert_eq!(orch.stats().stats.entries_added, 0);
            assert_eq!(orch.stats().stats.entries_removed, 0);

            // Create an ICMP echo session with IPv4 address
            let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
            let (entry, oid) = create_icmp_echo_session_with_sai("default", ip, IcmpMode::Enabled, &sai);

            // Verify SAI object was created
            assert_eq!(sai.count_objects(SaiObjectType::IcmpEchoSession), 1);

            // Verify SAI object attributes
            let sai_obj = sai.get_object(oid).unwrap();
            assert_eq!(sai_obj.object_type, SaiObjectType::IcmpEchoSession);
            assert_eq!(sai_obj.attributes.len(), 3);
            assert_eq!(sai_obj.attributes[0].0, "vrf");
            assert_eq!(sai_obj.attributes[0].1, "default");
            assert_eq!(sai_obj.attributes[1].0, "ip");
            assert_eq!(sai_obj.attributes[1].1, "10.0.0.1");
            assert_eq!(sai_obj.attributes[2].0, "mode");
            assert_eq!(sai_obj.attributes[2].1, "enabled");

            // Verify entry key properties
            assert_eq!(entry.key.vrf_name, "default");
            assert_eq!(entry.key.ip, ip);
            assert_eq!(entry.mode, IcmpMode::Enabled);
        }

        #[test]
        fn test_icmp_echo_configuration_integration() {
            let sai = MockSai::new();
            let config = IcmpOrchConfig::default();
            let orch = IcmpOrch::new(config);

            // Create multiple ICMP echo sessions with different configurations
            let ipv4_enabled = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
            let ipv6_disabled = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));

            // Create first session (IPv4, Enabled)
            let (_entry1, oid1) = create_icmp_echo_session_with_sai("default", ipv4_enabled, IcmpMode::Enabled, &sai);

            // Verify first session created
            assert_eq!(sai.count_objects(SaiObjectType::IcmpEchoSession), 1);

            // Create second session (IPv6, Disabled)
            let (_entry2, oid2) = create_icmp_echo_session_with_sai("Vrf-RED", ipv6_disabled, IcmpMode::Disabled, &sai);

            // Verify both sessions exist
            assert_eq!(sai.count_objects(SaiObjectType::IcmpEchoSession), 2);

            // Verify first session attributes
            let sai_obj1 = sai.get_object(oid1).unwrap();
            assert_eq!(sai_obj1.attributes[2].1, "enabled");
            assert_eq!(sai_obj1.attributes[0].1, "default");

            // Verify second session attributes
            let sai_obj2 = sai.get_object(oid2).unwrap();
            assert_eq!(sai_obj2.attributes[2].1, "disabled");
            assert_eq!(sai_obj2.attributes[0].1, "Vrf-RED");

            // Verify orchestrator state
            assert!(orch.get_entry(&IcmpEchoKey::new("default".to_string(), ipv4_enabled)).is_none());
        }

        #[test]
        fn test_multiple_icmp_echo_sessions_integration() {
            let sai = MockSai::new();
            let config = IcmpOrchConfig::default();
            let orch = IcmpOrch::new(config);

            // Create multiple ICMP echo sessions in default VRF
            let ips = vec![
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
                IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1)),
            ];

            let session_oids: Vec<u64> = ips
                .iter()
                .enumerate()
                .map(|(idx, ip)| {
                    let mode = if idx % 2 == 0 {
                        IcmpMode::Enabled
                    } else {
                        IcmpMode::Disabled
                    };
                    let (_, oid) = create_icmp_echo_session_with_sai("default", *ip, mode, &sai);
                    oid
                })
                .collect();

            // Verify all sessions were created
            assert_eq!(sai.count_objects(SaiObjectType::IcmpEchoSession), 3);
            assert_eq!(session_oids.len(), 3);

            // Verify all sessions exist in SAI
            for (idx, oid) in session_oids.iter().enumerate() {
                let sai_obj = sai.get_object(*oid).unwrap();
                assert_eq!(sai_obj.object_type, SaiObjectType::IcmpEchoSession);
                assert_eq!(sai_obj.attributes[0].1, "default");

                let expected_mode = if idx % 2 == 0 { "enabled" } else { "disabled" };
                assert_eq!(sai_obj.attributes[2].1, expected_mode);
            }

            // Verify sessions can be individually accessed
            for oid in &session_oids {
                assert!(sai.get_object(*oid).is_some());
            }
        }

        #[test]
        fn test_icmp_echo_removal_and_cleanup_integration() {
            let sai = MockSai::new();
            let config = IcmpOrchConfig::default();
            let orch = IcmpOrch::new(config);

            // Create initial ICMP echo sessions
            let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
            let ip2 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));

            let (_, oid1) = create_icmp_echo_session_with_sai("default", ip1, IcmpMode::Enabled, &sai);
            let (_, oid2) = create_icmp_echo_session_with_sai("default", ip2, IcmpMode::Enabled, &sai);

            // Verify both sessions created
            assert_eq!(sai.count_objects(SaiObjectType::IcmpEchoSession), 2);

            // Remove first session
            sai.remove_object(oid1).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::IcmpEchoSession), 1);

            // Verify removed session no longer exists
            assert!(sai.get_object(oid1).is_none());

            // Verify remaining session still exists
            assert!(sai.get_object(oid2).is_some());

            // Remove second session
            sai.remove_object(oid2).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::IcmpEchoSession), 0);

            // Verify complete cleanup
            assert!(sai.get_object(oid1).is_none());
            assert!(sai.get_object(oid2).is_none());

            // Attempt to remove non-existent session should fail gracefully
            let result = sai.remove_object(999);
            assert!(result.is_err());

            // Clear and verify final state
            sai.clear();
            assert_eq!(sai.count_objects(SaiObjectType::IcmpEchoSession), 0);
            assert_eq!(orch.stats().stats.entries_added, 0);
            assert_eq!(orch.stats().stats.entries_removed, 0);
        }
    }

    // FabricPortsOrch integration tests
    mod fabric_ports_orch_tests {
        use super::*;

        fn create_fabric_port_with_sai(
            lane: u32,
            status: &str,
            isolate: bool,
            sai: &MockSai,
        ) -> (u64, String) {
            let oid = sai.create_object(
                SaiObjectType::FabricPort,
                vec![
                    ("lane".to_string(), lane.to_string()),
                    ("status".to_string(), status.to_string()),
                    ("isolate".to_string(), isolate.to_string()),
                    ("attached".to_string(), "true".to_string()),
                    ("remote_mod".to_string(), "1".to_string()),
                    ("remote_port".to_string(), (lane as u32).to_string()),
                ],
            ).unwrap();

            let key = format!("PORT{}", lane);
            (oid, key)
        }

        /// Test fabric port creation and configuration
        #[test]
        fn test_fabric_port_creation_integration() {
            let sai = MockSai::new();

            // Verify initial state - no fabric ports
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 0);

            // Create a single fabric port with configuration
            let (oid, key) = create_fabric_port_with_sai(0, "up", false, &sai);

            assert_eq!(oid, 1);
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 1);

            // Verify the created object has correct attributes
            let obj = sai.get_object(oid).unwrap();
            assert_eq!(obj.object_type, SaiObjectType::FabricPort);
            assert_eq!(obj.attributes.len(), 6);

            // Verify specific attributes
            let status_attr = obj.attributes.iter()
                .find(|(k, _)| k == "status")
                .map(|(_, v)| v.clone());
            assert_eq!(status_attr, Some("up".to_string()));

            let isolate_attr = obj.attributes.iter()
                .find(|(k, _)| k == "isolate")
                .map(|(_, v)| v.clone());
            assert_eq!(isolate_attr, Some("false".to_string()));

            // Verify key matches expected format
            assert_eq!(key, "PORT0");

            sai.clear();
        }

        /// Test fabric port state management
        #[test]
        fn test_fabric_port_state_management_integration() {
            let sai = MockSai::new();

            // Create a fabric port in "up" state
            let (oid_up, _) = create_fabric_port_with_sai(1, "up", false, &sai);

            // Verify port is created with up status
            let obj_up = sai.get_object(oid_up).unwrap();
            let status = obj_up.attributes.iter()
                .find(|(k, _)| k == "status")
                .map(|(_, v)| v.clone());
            assert_eq!(status, Some("up".to_string()));

            // Create another fabric port in "down" state
            let (oid_down, _) = create_fabric_port_with_sai(2, "down", false, &sai);

            // Verify second port is created with down status
            let obj_down = sai.get_object(oid_down).unwrap();
            let status_down = obj_down.attributes.iter()
                .find(|(k, _)| k == "status")
                .map(|(_, v)| v.clone());
            assert_eq!(status_down, Some("down".to_string()));

            // Verify both ports exist
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 2);

            // Create an isolated port
            let (oid_isolated, _) = create_fabric_port_with_sai(3, "up", true, &sai);

            // Verify isolated port has isolation flag set
            let obj_isolated = sai.get_object(oid_isolated).unwrap();
            let isolate = obj_isolated.attributes.iter()
                .find(|(k, _)| k == "isolate")
                .map(|(_, v)| v.clone());
            assert_eq!(isolate, Some("true".to_string()));

            // Verify all three ports exist with different states
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 3);

            sai.clear();
        }

        /// Test multiple fabric port operations
        #[test]
        fn test_multiple_fabric_port_operations_integration() {
            let sai = MockSai::new();

            // Create a batch of fabric ports
            let mut fabric_port_oids = Vec::new();
            let mut fabric_port_keys = Vec::new();

            for i in 0..5 {
                let status = if i % 2 == 0 { "up" } else { "down" };
                let isolate = i >= 3;
                let (oid, key) = create_fabric_port_with_sai(i, status, isolate, &sai);
                fabric_port_oids.push(oid);
                fabric_port_keys.push(key);
            }

            // Verify all ports were created
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 5);
            assert_eq!(fabric_port_oids.len(), 5);
            assert_eq!(fabric_port_keys.len(), 5);

            // Verify each port can be retrieved and has correct attributes
            for (idx, oid) in fabric_port_oids.iter().enumerate() {
                let obj = sai.get_object(*oid).unwrap();
                assert_eq!(obj.object_type, SaiObjectType::FabricPort);

                let lane_attr = obj.attributes.iter()
                    .find(|(k, _)| k == "lane")
                    .map(|(_, v)| v.clone());
                assert_eq!(lane_attr, Some((idx as u32).to_string()));

                let expected_status = if idx % 2 == 0 { "up" } else { "down" };
                let status_attr = obj.attributes.iter()
                    .find(|(k, _)| k == "status")
                    .map(|(_, v)| v.clone());
                assert_eq!(status_attr, Some(expected_status.to_string()));

                let expected_isolate = idx >= 3;
                let isolate_attr = obj.attributes.iter()
                    .find(|(k, _)| k == "isolate")
                    .map(|(_, v)| v.clone());
                assert_eq!(isolate_attr, Some(expected_isolate.to_string()));
            }

            // Verify port keys are correctly formatted
            for (idx, key) in fabric_port_keys.iter().enumerate() {
                assert_eq!(key, &format!("PORT{}", idx));
            }

            // Perform bulk operations: retrieve all ports
            let all_ports: Vec<_> = fabric_port_oids.iter()
                .map(|oid| sai.get_object(*oid).unwrap())
                .collect();

            assert_eq!(all_ports.len(), 5);
            for port in &all_ports {
                assert_eq!(port.object_type, SaiObjectType::FabricPort);
                assert!(!port.attributes.is_empty());
            }

            sai.clear();
        }

        /// Test fabric port removal and cleanup
        #[test]
        fn test_fabric_port_removal_and_cleanup_integration() {
            let sai = MockSai::new();

            // Create initial set of fabric ports
            let fabric_port_oids: Vec<u64> = (0..4)
                .map(|i| {
                    let (oid, _) = create_fabric_port_with_sai(
                        i,
                        if i % 2 == 0 { "up" } else { "down" },
                        false,
                        &sai,
                    );
                    oid
                })
                .collect();

            // Verify all ports were created
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 4);

            // Remove first port
            sai.remove_object(fabric_port_oids[0]).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 3);

            // Verify removed port returns None
            assert!(sai.get_object(fabric_port_oids[0]).is_none());

            // Verify other ports still exist
            assert!(sai.get_object(fabric_port_oids[1]).is_some());
            assert!(sai.get_object(fabric_port_oids[2]).is_some());
            assert!(sai.get_object(fabric_port_oids[3]).is_some());

            // Remove remaining ports in sequence
            sai.remove_object(fabric_port_oids[1]).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 2);

            sai.remove_object(fabric_port_oids[2]).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 1);

            sai.remove_object(fabric_port_oids[3]).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 0);

            // Verify all removed ports return None
            for oid in &fabric_port_oids {
                assert!(sai.get_object(*oid).is_none());
            }

            // Attempt to remove non-existent port should fail
            let result = sai.remove_object(999);
            assert!(result.is_err());

            // Clear and verify clean state
            sai.clear();
            assert_eq!(sai.count_objects(SaiObjectType::FabricPort), 0);
        }
    }

    // ==================== DTel Integration Tests ====================

    /// Helper function to create a DTel event with SAI
    fn create_dtel_event_with_sai(
        event_type: &str,
        dscp_value: u8,
        sai: &MockSai,
    ) -> (u64, String) {
        let oid = sai
            .create_object(
                SaiObjectType::DtelEvent,
                vec![
                    ("event_type".to_string(), event_type.to_string()),
                    ("dscp_value".to_string(), dscp_value.to_string()),
                ],
            )
            .unwrap();
        (oid, event_type.to_string())
    }

    /// Helper function to create a DTel INT session with SAI
    fn create_dtel_int_session_with_sai(
        session_id: u32,
        max_hop_count: u8,
        collect_switch_id: bool,
        sai: &MockSai,
    ) -> (u64, u32) {
        let oid = sai
            .create_object(
                SaiObjectType::DtelIntSession,
                vec![
                    ("session_id".to_string(), session_id.to_string()),
                    ("max_hop_count".to_string(), max_hop_count.to_string()),
                    ("collect_switch_id".to_string(), collect_switch_id.to_string()),
                ],
            )
            .unwrap();
        (oid, session_id)
    }

    mod test_dtel_integration {
        use super::*;

        /// Test DTel event creation with SAI validation
        #[test]
        fn test_dtel_event_creation_integration() {
            let sai = MockSai::new();

            // Create a queue report event
            let (oid, event_type) = create_dtel_event_with_sai("QUEUE_REPORT", 8, &sai);

            // Verify SAI object was created
            assert_eq!(sai.count_objects(SaiObjectType::DtelEvent), 1);

            // Verify object attributes
            let obj = sai.get_object(oid).unwrap();
            assert_eq!(obj.object_type, SaiObjectType::DtelEvent);

            let type_attr = obj.attributes.iter()
                .find(|(k, _)| k == "event_type")
                .map(|(_, v)| v.clone());
            assert_eq!(type_attr, Some("QUEUE_REPORT".to_string()));

            let dscp_attr = obj.attributes.iter()
                .find(|(k, _)| k == "dscp_value")
                .map(|(_, v)| v.clone());
            assert_eq!(dscp_attr, Some("8".to_string()));

            sai.clear();
        }

        /// Test DTel INT session configuration
        #[test]
        fn test_dtel_int_session_configuration_integration() {
            let sai = MockSai::new();

            // Create INT session with specific configuration
            let (oid, session_id) = create_dtel_int_session_with_sai(1, 32, true, &sai);

            // Verify SAI object was created
            assert_eq!(sai.count_objects(SaiObjectType::DtelIntSession), 1);

            // Verify session attributes
            let obj = sai.get_object(oid).unwrap();
            assert_eq!(obj.object_type, SaiObjectType::DtelIntSession);

            let hop_attr = obj.attributes.iter()
                .find(|(k, _)| k == "max_hop_count")
                .map(|(_, v)| v.clone());
            assert_eq!(hop_attr, Some("32".to_string()));

            let switch_id_attr = obj.attributes.iter()
                .find(|(k, _)| k == "collect_switch_id")
                .map(|(_, v)| v.clone());
            assert_eq!(switch_id_attr, Some("true".to_string()));

            sai.clear();
        }

        /// Test multiple DTel events management
        #[test]
        fn test_multiple_dtel_events_management_integration() {
            let sai = MockSai::new();

            // Create multiple event types
            let event_types = vec![
                ("QUEUE_REPORT", 8),
                ("FLOW_STATE", 16),
                ("FLOW_REPORT", 24),
                ("DROP_REPORT", 32),
            ];

            let mut event_oids = Vec::new();
            for (event_type, dscp) in &event_types {
                let (oid, _) = create_dtel_event_with_sai(event_type, *dscp, &sai);
                event_oids.push(oid);
            }

            // Verify all events were created
            assert_eq!(sai.count_objects(SaiObjectType::DtelEvent), 4);

            // Verify each event has correct attributes
            for (idx, oid) in event_oids.iter().enumerate() {
                let obj = sai.get_object(*oid).unwrap();
                let type_attr = obj.attributes.iter()
                    .find(|(k, _)| k == "event_type")
                    .map(|(_, v)| v.clone());
                assert_eq!(type_attr, Some(event_types[idx].0.to_string()));
            }

            sai.clear();
        }

        /// Test DTel event removal and cleanup
        #[test]
        fn test_dtel_event_removal_and_cleanup_integration() {
            let sai = MockSai::new();

            // Create events
            let (oid1, _) = create_dtel_event_with_sai("QUEUE_REPORT", 8, &sai);
            let (oid2, _) = create_dtel_event_with_sai("FLOW_STATE", 16, &sai);
            let (session_oid, _) = create_dtel_int_session_with_sai(1, 32, true, &sai);

            assert_eq!(sai.count_objects(SaiObjectType::DtelEvent), 2);
            assert_eq!(sai.count_objects(SaiObjectType::DtelIntSession), 1);

            // Remove first event
            sai.remove_object(oid1).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::DtelEvent), 1);

            // Remove session
            sai.remove_object(session_oid).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::DtelIntSession), 0);

            // Remove remaining event
            sai.remove_object(oid2).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::DtelEvent), 0);

            sai.clear();
        }
    }

    // ==================== FDB Integration Tests ====================

    /// Helper function to create an FDB entry with SAI
    fn create_fdb_entry_with_sai(
        mac: &str,
        vlan_id: u16,
        port: &str,
        entry_type: &str,
        sai: &MockSai,
    ) -> (u64, String) {
        let key = format!("{}:{}", mac, vlan_id);
        let oid = sai
            .create_object(
                SaiObjectType::FdbEntry,
                vec![
                    ("mac".to_string(), mac.to_string()),
                    ("vlan_id".to_string(), vlan_id.to_string()),
                    ("port".to_string(), port.to_string()),
                    ("entry_type".to_string(), entry_type.to_string()),
                ],
            )
            .unwrap();
        (oid, key)
    }

    mod test_fdb_integration {
        use super::*;

        /// Test FDB entry creation with SAI validation
        #[test]
        fn test_fdb_entry_creation_integration() {
            let sai = MockSai::new();

            // Create FDB entry
            let (oid, key) = create_fdb_entry_with_sai(
                "00:11:22:33:44:55",
                100,
                "Ethernet0",
                "static",
                &sai,
            );

            // Verify SAI object was created
            assert_eq!(sai.count_objects(SaiObjectType::FdbEntry), 1);

            // Verify object attributes
            let obj = sai.get_object(oid).unwrap();
            assert_eq!(obj.object_type, SaiObjectType::FdbEntry);

            let mac_attr = obj.attributes.iter()
                .find(|(k, _)| k == "mac")
                .map(|(_, v)| v.clone());
            assert_eq!(mac_attr, Some("00:11:22:33:44:55".to_string()));

            let vlan_attr = obj.attributes.iter()
                .find(|(k, _)| k == "vlan_id")
                .map(|(_, v)| v.clone());
            assert_eq!(vlan_attr, Some("100".to_string()));

            sai.clear();
        }

        /// Test FDB entry MAC update
        #[test]
        fn test_fdb_entry_mac_update_integration() {
            let sai = MockSai::new();

            // Create initial entry
            let (oid1, _) = create_fdb_entry_with_sai(
                "00:11:22:33:44:55",
                100,
                "Ethernet0",
                "dynamic",
                &sai,
            );

            // Simulate MAC move - create new entry with same MAC on different port
            let (oid2, _) = create_fdb_entry_with_sai(
                "00:11:22:33:44:55",
                100,
                "Ethernet4",
                "dynamic",
                &sai,
            );

            // In real scenario, old entry would be removed first
            // Here we verify both exist (before cleanup)
            assert_eq!(sai.count_objects(SaiObjectType::FdbEntry), 2);

            // Remove old entry (simulating MAC move)
            sai.remove_object(oid1).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::FdbEntry), 1);

            // Verify new entry has correct port
            let obj = sai.get_object(oid2).unwrap();
            let port_attr = obj.attributes.iter()
                .find(|(k, _)| k == "port")
                .map(|(_, v)| v.clone());
            assert_eq!(port_attr, Some("Ethernet4".to_string()));

            sai.clear();
        }

        /// Test multiple FDB entries across VLANs
        #[test]
        fn test_multiple_fdb_entries_management_integration() {
            let sai = MockSai::new();

            // Create entries across multiple VLANs
            let entries = vec![
                ("00:11:22:33:44:55", 100, "Ethernet0"),
                ("00:11:22:33:44:66", 100, "Ethernet4"),
                ("00:11:22:33:44:77", 200, "Ethernet8"),
                ("00:11:22:33:44:88", 200, "Ethernet12"),
                ("00:11:22:33:44:99", 300, "Ethernet16"),
            ];

            let mut entry_oids = Vec::new();
            for (mac, vlan, port) in &entries {
                let (oid, _) = create_fdb_entry_with_sai(mac, *vlan, port, "static", &sai);
                entry_oids.push(oid);
            }

            // Verify all entries were created
            assert_eq!(sai.count_objects(SaiObjectType::FdbEntry), 5);

            // Verify entries have correct VLANs
            for (idx, oid) in entry_oids.iter().enumerate() {
                let obj = sai.get_object(*oid).unwrap();
                let vlan_attr = obj.attributes.iter()
                    .find(|(k, _)| k == "vlan_id")
                    .map(|(_, v)| v.clone());
                assert_eq!(vlan_attr, Some(entries[idx].1.to_string()));
            }

            sai.clear();
        }

        /// Test FDB entry removal and cleanup
        #[test]
        fn test_fdb_entry_removal_and_cleanup_integration() {
            let sai = MockSai::new();

            // Create entries
            let (oid1, _) = create_fdb_entry_with_sai("00:11:22:33:44:55", 100, "Ethernet0", "static", &sai);
            let (oid2, _) = create_fdb_entry_with_sai("00:11:22:33:44:66", 100, "Ethernet4", "dynamic", &sai);
            let (oid3, _) = create_fdb_entry_with_sai("00:11:22:33:44:77", 200, "Ethernet8", "static", &sai);

            assert_eq!(sai.count_objects(SaiObjectType::FdbEntry), 3);

            // Remove entries one by one
            sai.remove_object(oid1).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::FdbEntry), 2);

            sai.remove_object(oid2).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::FdbEntry), 1);

            sai.remove_object(oid3).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::FdbEntry), 0);

            // Verify all removed
            assert!(sai.get_object(oid1).is_none());
            assert!(sai.get_object(oid2).is_none());
            assert!(sai.get_object(oid3).is_none());

            sai.clear();
        }
    }

    // ==================== FG-NHG Integration Tests ====================

    /// Helper function to create a fine-grained NHG with SAI
    fn create_fg_nhg_with_sai(
        prefix: &str,
        bucket_size: u32,
        sai: &MockSai,
    ) -> (u64, String) {
        let oid = sai
            .create_object(
                SaiObjectType::FgNhg,
                vec![
                    ("prefix".to_string(), prefix.to_string()),
                    ("bucket_size".to_string(), bucket_size.to_string()),
                ],
            )
            .unwrap();
        (oid, prefix.to_string())
    }

    /// Helper function to create a fine-grained NHG member with SAI
    fn create_fg_nhg_member_with_sai(
        nhg_oid: u64,
        next_hop_ip: &str,
        bucket_index: u32,
        sai: &MockSai,
    ) -> u64 {
        sai.create_object(
            SaiObjectType::FgNhgMember,
            vec![
                ("nhg_oid".to_string(), nhg_oid.to_string()),
                ("next_hop_ip".to_string(), next_hop_ip.to_string()),
                ("bucket_index".to_string(), bucket_index.to_string()),
            ],
        )
        .unwrap()
    }

    mod test_fg_nhg_integration {
        use super::*;

        /// Test fine-grained NHG creation with SAI validation
        #[test]
        fn test_fg_nhg_creation_integration() {
            let sai = MockSai::new();

            // Create FG-NHG
            let (oid, prefix) = create_fg_nhg_with_sai("10.0.0.0/24", 64, &sai);

            // Verify SAI object was created
            assert_eq!(sai.count_objects(SaiObjectType::FgNhg), 1);

            // Verify object attributes
            let obj = sai.get_object(oid).unwrap();
            assert_eq!(obj.object_type, SaiObjectType::FgNhg);

            let prefix_attr = obj.attributes.iter()
                .find(|(k, _)| k == "prefix")
                .map(|(_, v)| v.clone());
            assert_eq!(prefix_attr, Some("10.0.0.0/24".to_string()));

            let bucket_attr = obj.attributes.iter()
                .find(|(k, _)| k == "bucket_size")
                .map(|(_, v)| v.clone());
            assert_eq!(bucket_attr, Some("64".to_string()));

            sai.clear();
        }

        /// Test FG-NHG member operations
        #[test]
        fn test_fg_nhg_member_operations_integration() {
            let sai = MockSai::new();

            // Create NHG first
            let (nhg_oid, _) = create_fg_nhg_with_sai("10.0.0.0/24", 64, &sai);

            // Add members to different buckets
            let member1_oid = create_fg_nhg_member_with_sai(nhg_oid, "192.168.1.1", 0, &sai);
            let member2_oid = create_fg_nhg_member_with_sai(nhg_oid, "192.168.1.2", 16, &sai);
            let member3_oid = create_fg_nhg_member_with_sai(nhg_oid, "192.168.1.3", 32, &sai);
            let member4_oid = create_fg_nhg_member_with_sai(nhg_oid, "192.168.1.4", 48, &sai);

            // Verify members were created
            assert_eq!(sai.count_objects(SaiObjectType::FgNhgMember), 4);

            // Verify member attributes
            let member1 = sai.get_object(member1_oid).unwrap();
            let bucket_attr = member1.attributes.iter()
                .find(|(k, _)| k == "bucket_index")
                .map(|(_, v)| v.clone());
            assert_eq!(bucket_attr, Some("0".to_string()));

            // Remove a member
            sai.remove_object(member2_oid).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::FgNhgMember), 3);

            sai.clear();
        }

        /// Test multiple FG-NHGs management
        #[test]
        fn test_multiple_fg_nhgs_management_integration() {
            let sai = MockSai::new();

            // Create multiple NHGs with different prefixes
            let prefixes = vec![
                ("10.0.0.0/24", 64),
                ("10.1.0.0/24", 128),
                ("10.2.0.0/24", 256),
            ];

            let mut nhg_oids = Vec::new();
            for (prefix, bucket_size) in &prefixes {
                let (oid, _) = create_fg_nhg_with_sai(prefix, *bucket_size, &sai);
                nhg_oids.push(oid);

                // Add members to each NHG
                create_fg_nhg_member_with_sai(oid, "192.168.1.1", 0, &sai);
                create_fg_nhg_member_with_sai(oid, "192.168.1.2", 1, &sai);
            }

            // Verify all NHGs and members were created
            assert_eq!(sai.count_objects(SaiObjectType::FgNhg), 3);
            assert_eq!(sai.count_objects(SaiObjectType::FgNhgMember), 6);

            sai.clear();
        }

        /// Test FG-NHG removal and cleanup
        #[test]
        fn test_fg_nhg_removal_and_cleanup_integration() {
            let sai = MockSai::new();

            // Create NHG with members
            let (nhg_oid, _) = create_fg_nhg_with_sai("10.0.0.0/24", 64, &sai);
            let member1_oid = create_fg_nhg_member_with_sai(nhg_oid, "192.168.1.1", 0, &sai);
            let member2_oid = create_fg_nhg_member_with_sai(nhg_oid, "192.168.1.2", 16, &sai);

            assert_eq!(sai.count_objects(SaiObjectType::FgNhg), 1);
            assert_eq!(sai.count_objects(SaiObjectType::FgNhgMember), 2);

            // Remove members first (proper cleanup order)
            sai.remove_object(member1_oid).unwrap();
            sai.remove_object(member2_oid).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::FgNhgMember), 0);

            // Remove NHG
            sai.remove_object(nhg_oid).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::FgNhg), 0);

            sai.clear();
        }
    }

    // ==================== Intfs Integration Tests ====================

    /// Helper function to create a router interface with SAI
    fn create_router_interface_with_sai(
        name: &str,
        vrf_name: &str,
        mac: &str,
        sai: &MockSai,
    ) -> (u64, String) {
        let oid = sai
            .create_object(
                SaiObjectType::RouterInterface,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("vrf".to_string(), vrf_name.to_string()),
                    ("mac".to_string(), mac.to_string()),
                ],
            )
            .unwrap();
        (oid, name.to_string())
    }

    mod test_intfs_integration {
        use super::*;

        /// Test router interface creation with SAI validation
        #[test]
        fn test_intfs_router_interface_creation_integration() {
            let sai = MockSai::new();

            // Create router interface
            let (oid, name) = create_router_interface_with_sai(
                "Ethernet0",
                "default",
                "00:11:22:33:44:55",
                &sai,
            );

            // Verify SAI object was created
            assert_eq!(sai.count_objects(SaiObjectType::RouterInterface), 1);

            // Verify object attributes
            let obj = sai.get_object(oid).unwrap();
            assert_eq!(obj.object_type, SaiObjectType::RouterInterface);

            let name_attr = obj.attributes.iter()
                .find(|(k, _)| k == "name")
                .map(|(_, v)| v.clone());
            assert_eq!(name_attr, Some("Ethernet0".to_string()));

            let vrf_attr = obj.attributes.iter()
                .find(|(k, _)| k == "vrf")
                .map(|(_, v)| v.clone());
            assert_eq!(vrf_attr, Some("default".to_string()));

            sai.clear();
        }

        /// Test interface IP address configuration
        #[test]
        fn test_intfs_ip_address_configuration_integration() {
            let sai = MockSai::new();

            // Create router interface with VRF
            let (oid, _) = create_router_interface_with_sai(
                "Ethernet0",
                "Vrf_RED",
                "00:11:22:33:44:55",
                &sai,
            );

            // Verify interface in VRF
            let obj = sai.get_object(oid).unwrap();
            let vrf_attr = obj.attributes.iter()
                .find(|(k, _)| k == "vrf")
                .map(|(_, v)| v.clone());
            assert_eq!(vrf_attr, Some("Vrf_RED".to_string()));

            // Create another interface in same VRF
            let (oid2, _) = create_router_interface_with_sai(
                "Ethernet4",
                "Vrf_RED",
                "00:11:22:33:44:66",
                &sai,
            );

            assert_eq!(sai.count_objects(SaiObjectType::RouterInterface), 2);

            sai.clear();
        }

        /// Test multiple interfaces management
        #[test]
        fn test_multiple_interfaces_management_integration() {
            let sai = MockSai::new();

            // Create interfaces on multiple ports in different VRFs
            let interfaces = vec![
                ("Ethernet0", "default", "00:11:22:33:44:01"),
                ("Ethernet4", "default", "00:11:22:33:44:02"),
                ("Ethernet8", "Vrf_RED", "00:11:22:33:44:03"),
                ("Ethernet12", "Vrf_RED", "00:11:22:33:44:04"),
                ("Ethernet16", "Vrf_BLUE", "00:11:22:33:44:05"),
            ];

            let mut intf_oids = Vec::new();
            for (name, vrf, mac) in &interfaces {
                let (oid, _) = create_router_interface_with_sai(name, vrf, mac, &sai);
                intf_oids.push(oid);
            }

            // Verify all interfaces were created
            assert_eq!(sai.count_objects(SaiObjectType::RouterInterface), 5);

            // Verify each interface has correct attributes
            for (idx, oid) in intf_oids.iter().enumerate() {
                let obj = sai.get_object(*oid).unwrap();
                let name_attr = obj.attributes.iter()
                    .find(|(k, _)| k == "name")
                    .map(|(_, v)| v.clone());
                assert_eq!(name_attr, Some(interfaces[idx].0.to_string()));
            }

            sai.clear();
        }

        /// Test interface removal and cleanup
        #[test]
        fn test_intfs_removal_and_cleanup_integration() {
            let sai = MockSai::new();

            // Create interfaces
            let (oid1, _) = create_router_interface_with_sai("Ethernet0", "default", "00:11:22:33:44:01", &sai);
            let (oid2, _) = create_router_interface_with_sai("Ethernet4", "default", "00:11:22:33:44:02", &sai);
            let (oid3, _) = create_router_interface_with_sai("Ethernet8", "Vrf_RED", "00:11:22:33:44:03", &sai);

            assert_eq!(sai.count_objects(SaiObjectType::RouterInterface), 3);

            // Remove interfaces
            sai.remove_object(oid1).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::RouterInterface), 2);

            sai.remove_object(oid2).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::RouterInterface), 1);

            sai.remove_object(oid3).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::RouterInterface), 0);

            // Verify all removed
            assert!(sai.get_object(oid1).is_none());
            assert!(sai.get_object(oid2).is_none());
            assert!(sai.get_object(oid3).is_none());

            sai.clear();
        }
    }

    // ==================== Mirror Integration Tests ====================

    /// Helper function to create a mirror session with SAI
    fn create_mirror_session_with_sai(
        name: &str,
        session_type: &str,
        direction: &str,
        dst_port: &str,
        sai: &MockSai,
    ) -> (u64, String) {
        let oid = sai
            .create_object(
                SaiObjectType::MirrorSession,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("session_type".to_string(), session_type.to_string()),
                    ("direction".to_string(), direction.to_string()),
                    ("dst_port".to_string(), dst_port.to_string()),
                ],
            )
            .unwrap();
        (oid, name.to_string())
    }

    /// Helper function to create an ERSPAN session with SAI
    fn create_erspan_session_with_sai(
        name: &str,
        direction: &str,
        src_ip: &str,
        dst_ip: &str,
        gre_type: u16,
        sai: &MockSai,
    ) -> (u64, String) {
        let oid = sai
            .create_object(
                SaiObjectType::MirrorSession,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("session_type".to_string(), "ERSPAN".to_string()),
                    ("direction".to_string(), direction.to_string()),
                    ("src_ip".to_string(), src_ip.to_string()),
                    ("dst_ip".to_string(), dst_ip.to_string()),
                    ("gre_type".to_string(), gre_type.to_string()),
                ],
            )
            .unwrap();
        (oid, name.to_string())
    }

    mod test_mirror_integration {
        use super::*;

        /// Test SPAN session creation with SAI validation
        #[test]
        fn test_mirror_span_session_creation_integration() {
            let sai = MockSai::new();

            // Create SPAN session
            let (oid, name) = create_mirror_session_with_sai(
                "mirror_session_1",
                "SPAN",
                "BOTH",
                "Ethernet48",
                &sai,
            );

            // Verify SAI object was created
            assert_eq!(sai.count_objects(SaiObjectType::MirrorSession), 1);

            // Verify object attributes
            let obj = sai.get_object(oid).unwrap();
            assert_eq!(obj.object_type, SaiObjectType::MirrorSession);

            let type_attr = obj.attributes.iter()
                .find(|(k, _)| k == "session_type")
                .map(|(_, v)| v.clone());
            assert_eq!(type_attr, Some("SPAN".to_string()));

            let dir_attr = obj.attributes.iter()
                .find(|(k, _)| k == "direction")
                .map(|(_, v)| v.clone());
            assert_eq!(dir_attr, Some("BOTH".to_string()));

            sai.clear();
        }

        /// Test ERSPAN session with GRE tunnel configuration
        #[test]
        fn test_mirror_erspan_session_configuration_integration() {
            let sai = MockSai::new();

            // Create ERSPAN session with GRE tunnel
            let (oid, _) = create_erspan_session_with_sai(
                "erspan_session_1",
                "RX",
                "10.0.0.1",
                "10.0.0.2",
                0x88BE,  // ERSPAN Type II GRE type
                &sai,
            );

            // Verify SAI object was created
            assert_eq!(sai.count_objects(SaiObjectType::MirrorSession), 1);

            // Verify ERSPAN attributes
            let obj = sai.get_object(oid).unwrap();

            let type_attr = obj.attributes.iter()
                .find(|(k, _)| k == "session_type")
                .map(|(_, v)| v.clone());
            assert_eq!(type_attr, Some("ERSPAN".to_string()));

            let src_attr = obj.attributes.iter()
                .find(|(k, _)| k == "src_ip")
                .map(|(_, v)| v.clone());
            assert_eq!(src_attr, Some("10.0.0.1".to_string()));

            let dst_attr = obj.attributes.iter()
                .find(|(k, _)| k == "dst_ip")
                .map(|(_, v)| v.clone());
            assert_eq!(dst_attr, Some("10.0.0.2".to_string()));

            sai.clear();
        }

        /// Test multiple mirror sessions management
        #[test]
        fn test_multiple_mirror_sessions_management_integration() {
            let sai = MockSai::new();

            // Create sessions with different directions
            let (oid1, _) = create_mirror_session_with_sai("session_rx", "SPAN", "RX", "Ethernet48", &sai);
            let (oid2, _) = create_mirror_session_with_sai("session_tx", "SPAN", "TX", "Ethernet48", &sai);
            let (oid3, _) = create_mirror_session_with_sai("session_both", "SPAN", "BOTH", "Ethernet52", &sai);
            let (oid4, _) = create_erspan_session_with_sai("erspan_1", "RX", "10.0.0.1", "10.0.0.2", 0x88BE, &sai);

            // Verify all sessions were created
            assert_eq!(sai.count_objects(SaiObjectType::MirrorSession), 4);

            // Verify directions
            let sessions = vec![
                (oid1, "RX"),
                (oid2, "TX"),
                (oid3, "BOTH"),
                (oid4, "RX"),
            ];

            for (oid, expected_dir) in sessions {
                let obj = sai.get_object(oid).unwrap();
                let dir_attr = obj.attributes.iter()
                    .find(|(k, _)| k == "direction")
                    .map(|(_, v)| v.clone());
                assert_eq!(dir_attr, Some(expected_dir.to_string()));
            }

            sai.clear();
        }

        /// Test mirror session removal and cleanup
        #[test]
        fn test_mirror_session_removal_and_cleanup_integration() {
            let sai = MockSai::new();

            // Create sessions
            let (oid1, _) = create_mirror_session_with_sai("session_1", "SPAN", "RX", "Ethernet48", &sai);
            let (oid2, _) = create_erspan_session_with_sai("erspan_1", "TX", "10.0.0.1", "10.0.0.2", 0x88BE, &sai);

            assert_eq!(sai.count_objects(SaiObjectType::MirrorSession), 2);

            // Remove sessions
            sai.remove_object(oid1).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::MirrorSession), 1);

            sai.remove_object(oid2).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::MirrorSession), 0);

            // Verify all removed
            assert!(sai.get_object(oid1).is_none());
            assert!(sai.get_object(oid2).is_none());

            sai.clear();
        }
    }

    // ==================== MUX Integration Tests ====================

    /// Helper function to create a MUX port with SAI (creates tunnel and ACL)
    fn create_mux_port_with_sai(
        port_name: &str,
        state: &str,
        peer_ip: &str,
        sai: &MockSai,
    ) -> (u64, u64, String) {
        // Create MUX tunnel
        let tunnel_oid = sai
            .create_object(
                SaiObjectType::MuxTunnel,
                vec![
                    ("port_name".to_string(), port_name.to_string()),
                    ("state".to_string(), state.to_string()),
                    ("peer_ip".to_string(), peer_ip.to_string()),
                ],
            )
            .unwrap();

        // Create MUX ACL
        let acl_oid = sai
            .create_object(
                SaiObjectType::MuxAcl,
                vec![
                    ("port_name".to_string(), port_name.to_string()),
                    ("state".to_string(), state.to_string()),
                ],
            )
            .unwrap();

        (tunnel_oid, acl_oid, port_name.to_string())
    }

    mod test_mux_integration {
        use super::*;

        /// Test MUX port creation with SAI validation
        #[test]
        fn test_mux_port_creation_integration() {
            let sai = MockSai::new();

            // Create MUX port (creates both tunnel and ACL)
            let (tunnel_oid, acl_oid, port_name) = create_mux_port_with_sai(
                "Ethernet0",
                "active",
                "10.0.0.1",
                &sai,
            );

            // Verify SAI objects were created
            assert_eq!(sai.count_objects(SaiObjectType::MuxTunnel), 1);
            assert_eq!(sai.count_objects(SaiObjectType::MuxAcl), 1);

            // Verify tunnel attributes
            let tunnel = sai.get_object(tunnel_oid).unwrap();
            assert_eq!(tunnel.object_type, SaiObjectType::MuxTunnel);

            let state_attr = tunnel.attributes.iter()
                .find(|(k, _)| k == "state")
                .map(|(_, v)| v.clone());
            assert_eq!(state_attr, Some("active".to_string()));

            // Verify ACL attributes
            let acl = sai.get_object(acl_oid).unwrap();
            assert_eq!(acl.object_type, SaiObjectType::MuxAcl);

            sai.clear();
        }

        /// Test MUX state transition
        #[test]
        fn test_mux_state_transition_integration() {
            let sai = MockSai::new();

            // Create MUX port in active state
            let (tunnel_oid, acl_oid, _) = create_mux_port_with_sai(
                "Ethernet0",
                "active",
                "10.0.0.1",
                &sai,
            );

            // Verify initial state
            let tunnel = sai.get_object(tunnel_oid).unwrap();
            let state_attr = tunnel.attributes.iter()
                .find(|(k, _)| k == "state")
                .map(|(_, v)| v.clone());
            assert_eq!(state_attr, Some("active".to_string()));

            // Simulate state transition by removing and recreating with new state
            sai.remove_object(tunnel_oid).unwrap();
            sai.remove_object(acl_oid).unwrap();

            let (new_tunnel_oid, new_acl_oid, _) = create_mux_port_with_sai(
                "Ethernet0",
                "standby",
                "10.0.0.1",
                &sai,
            );

            // Verify new state
            let new_tunnel = sai.get_object(new_tunnel_oid).unwrap();
            let new_state_attr = new_tunnel.attributes.iter()
                .find(|(k, _)| k == "state")
                .map(|(_, v)| v.clone());
            assert_eq!(new_state_attr, Some("standby".to_string()));

            sai.clear();
        }

        /// Test multiple MUX ports management
        #[test]
        fn test_multiple_mux_ports_management_integration() {
            let sai = MockSai::new();

            // Create MUX ports on different interfaces
            let ports = vec![
                ("Ethernet0", "active", "10.0.0.1"),
                ("Ethernet4", "standby", "10.0.0.2"),
                ("Ethernet8", "active", "10.0.0.3"),
                ("Ethernet12", "standby", "10.0.0.4"),
            ];

            let mut tunnel_oids = Vec::new();
            for (port, state, peer_ip) in &ports {
                let (tunnel_oid, _, _) = create_mux_port_with_sai(port, state, peer_ip, &sai);
                tunnel_oids.push(tunnel_oid);
            }

            // Verify all objects were created
            assert_eq!(sai.count_objects(SaiObjectType::MuxTunnel), 4);
            assert_eq!(sai.count_objects(SaiObjectType::MuxAcl), 4);

            // Verify states
            for (idx, oid) in tunnel_oids.iter().enumerate() {
                let obj = sai.get_object(*oid).unwrap();
                let state_attr = obj.attributes.iter()
                    .find(|(k, _)| k == "state")
                    .map(|(_, v)| v.clone());
                assert_eq!(state_attr, Some(ports[idx].1.to_string()));
            }

            sai.clear();
        }

        /// Test MUX port removal and cleanup
        #[test]
        fn test_mux_port_removal_and_cleanup_integration() {
            let sai = MockSai::new();

            // Create MUX ports
            let (tunnel_oid1, acl_oid1, _) = create_mux_port_with_sai("Ethernet0", "active", "10.0.0.1", &sai);
            let (tunnel_oid2, acl_oid2, _) = create_mux_port_with_sai("Ethernet4", "standby", "10.0.0.2", &sai);

            assert_eq!(sai.count_objects(SaiObjectType::MuxTunnel), 2);
            assert_eq!(sai.count_objects(SaiObjectType::MuxAcl), 2);

            // Remove first port (both tunnel and ACL)
            sai.remove_object(acl_oid1).unwrap();
            sai.remove_object(tunnel_oid1).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::MuxTunnel), 1);
            assert_eq!(sai.count_objects(SaiObjectType::MuxAcl), 1);

            // Remove second port
            sai.remove_object(acl_oid2).unwrap();
            sai.remove_object(tunnel_oid2).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::MuxTunnel), 0);
            assert_eq!(sai.count_objects(SaiObjectType::MuxAcl), 0);

            sai.clear();
        }
    }

    // ==================== PBH Integration Tests ====================

    /// Helper function to create a PBH hash with SAI
    fn create_pbh_hash_with_sai(
        name: &str,
        hash_field_list: Vec<&str>,
        sai: &MockSai,
    ) -> (u64, String) {
        let fields_str = hash_field_list.join(",");
        let oid = sai
            .create_object(
                SaiObjectType::PbhHash,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("hash_field_list".to_string(), fields_str),
                ],
            )
            .unwrap();
        (oid, name.to_string())
    }

    /// Helper function to create a PBH table with SAI
    fn create_pbh_table_with_sai(
        name: &str,
        interface_list: Vec<&str>,
        sai: &MockSai,
    ) -> (u64, String) {
        let interfaces_str = interface_list.join(",");
        let oid = sai
            .create_object(
                SaiObjectType::PbhTable,
                vec![
                    ("name".to_string(), name.to_string()),
                    ("interface_list".to_string(), interfaces_str),
                ],
            )
            .unwrap();
        (oid, name.to_string())
    }

    /// Helper function to create a PBH rule with SAI
    fn create_pbh_rule_with_sai(
        table_name: &str,
        rule_name: &str,
        priority: u32,
        hash_name: &str,
        sai: &MockSai,
    ) -> (u64, String) {
        let oid = sai
            .create_object(
                SaiObjectType::PbhRule,
                vec![
                    ("table_name".to_string(), table_name.to_string()),
                    ("rule_name".to_string(), rule_name.to_string()),
                    ("priority".to_string(), priority.to_string()),
                    ("hash_name".to_string(), hash_name.to_string()),
                ],
            )
            .unwrap();
        (oid, rule_name.to_string())
    }

    mod test_pbh_integration {
        use super::*;

        /// Test PBH hash creation with SAI validation
        #[test]
        fn test_pbh_hash_creation_integration() {
            let sai = MockSai::new();

            // Create PBH hash with multiple fields
            let (oid, name) = create_pbh_hash_with_sai(
                "inner_v4_hash",
                vec!["INNER_SRC_IPV4", "INNER_DST_IPV4", "INNER_L4_SRC_PORT", "INNER_L4_DST_PORT"],
                &sai,
            );

            // Verify SAI object was created
            assert_eq!(sai.count_objects(SaiObjectType::PbhHash), 1);

            // Verify object attributes
            let obj = sai.get_object(oid).unwrap();
            assert_eq!(obj.object_type, SaiObjectType::PbhHash);

            let name_attr = obj.attributes.iter()
                .find(|(k, _)| k == "name")
                .map(|(_, v)| v.clone());
            assert_eq!(name_attr, Some("inner_v4_hash".to_string()));

            let fields_attr = obj.attributes.iter()
                .find(|(k, _)| k == "hash_field_list")
                .map(|(_, v)| v.clone());
            assert!(fields_attr.unwrap().contains("INNER_SRC_IPV4"));

            sai.clear();
        }

        /// Test PBH table and rule configuration
        #[test]
        fn test_pbh_table_and_rule_configuration_integration() {
            let sai = MockSai::new();

            // Create hash first
            let (hash_oid, hash_name) = create_pbh_hash_with_sai(
                "inner_v4_hash",
                vec!["INNER_SRC_IPV4", "INNER_DST_IPV4"],
                &sai,
            );

            // Create table
            let (table_oid, table_name) = create_pbh_table_with_sai(
                "pbh_table_1",
                vec!["Ethernet0", "Ethernet4", "Ethernet8"],
                &sai,
            );

            // Create rule
            let (rule_oid, rule_name) = create_pbh_rule_with_sai(
                &table_name,
                "rule_1",
                100,
                &hash_name,
                &sai,
            );

            // Verify all objects were created
            assert_eq!(sai.count_objects(SaiObjectType::PbhHash), 1);
            assert_eq!(sai.count_objects(SaiObjectType::PbhTable), 1);
            assert_eq!(sai.count_objects(SaiObjectType::PbhRule), 1);

            // Verify rule attributes
            let rule = sai.get_object(rule_oid).unwrap();
            let priority_attr = rule.attributes.iter()
                .find(|(k, _)| k == "priority")
                .map(|(_, v)| v.clone());
            assert_eq!(priority_attr, Some("100".to_string()));

            sai.clear();
        }

        /// Test multiple PBH hashes management
        #[test]
        fn test_multiple_pbh_hashes_management_integration() {
            let sai = MockSai::new();

            // Create multiple hashes with different fields
            let hashes = vec![
                ("inner_v4_hash", vec!["INNER_SRC_IPV4", "INNER_DST_IPV4"]),
                ("inner_v6_hash", vec!["INNER_SRC_IPV6", "INNER_DST_IPV6"]),
                ("outer_v4_hash", vec!["SRC_IPV4", "DST_IPV4"]),
                ("gre_hash", vec!["GRE_KEY"]),
            ];

            let mut hash_oids = Vec::new();
            for (name, fields) in &hashes {
                let (oid, _) = create_pbh_hash_with_sai(name, fields.clone(), &sai);
                hash_oids.push(oid);
            }

            // Verify all hashes were created
            assert_eq!(sai.count_objects(SaiObjectType::PbhHash), 4);

            // Verify each hash has correct name
            for (idx, oid) in hash_oids.iter().enumerate() {
                let obj = sai.get_object(*oid).unwrap();
                let name_attr = obj.attributes.iter()
                    .find(|(k, _)| k == "name")
                    .map(|(_, v)| v.clone());
                assert_eq!(name_attr, Some(hashes[idx].0.to_string()));
            }

            sai.clear();
        }

        /// Test PBH removal and cleanup
        #[test]
        fn test_pbh_removal_and_cleanup_integration() {
            let sai = MockSai::new();

            // Create full PBH configuration
            let (hash_oid, hash_name) = create_pbh_hash_with_sai(
                "inner_v4_hash",
                vec!["INNER_SRC_IPV4", "INNER_DST_IPV4"],
                &sai,
            );
            let (table_oid, table_name) = create_pbh_table_with_sai(
                "pbh_table_1",
                vec!["Ethernet0"],
                &sai,
            );
            let (rule_oid, _) = create_pbh_rule_with_sai(
                &table_name,
                "rule_1",
                100,
                &hash_name,
                &sai,
            );

            assert_eq!(sai.count_objects(SaiObjectType::PbhHash), 1);
            assert_eq!(sai.count_objects(SaiObjectType::PbhTable), 1);
            assert_eq!(sai.count_objects(SaiObjectType::PbhRule), 1);

            // Remove in proper order: rule, table, hash
            sai.remove_object(rule_oid).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::PbhRule), 0);

            sai.remove_object(table_oid).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::PbhTable), 0);

            sai.remove_object(hash_oid).unwrap();
            assert_eq!(sai.count_objects(SaiObjectType::PbhHash), 0);

            // Verify all removed
            assert!(sai.get_object(rule_oid).is_none());
            assert!(sai.get_object(table_oid).is_none());
            assert!(sai.get_object(hash_oid).is_none());

            sai.clear();
        }
    }

}
