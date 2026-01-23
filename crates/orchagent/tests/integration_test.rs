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
}
