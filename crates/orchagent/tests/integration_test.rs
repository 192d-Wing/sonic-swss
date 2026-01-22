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

            let (pool, oid) = create_pool_with_sai("ingress_lossless_pool", 10485760, &sai);
            orch.add_pool(pool).unwrap();

            orch.increment_pool_ref("ingress_lossless_pool").unwrap();
            orch.decrement_pool_ref("ingress_lossless_pool").unwrap();

            let removed = orch.remove_pool("ingress_lossless_pool").unwrap();
            sai.remove_object(removed.sai_oid).unwrap();

            assert_eq!(orch.pool_count(), 0);
            assert_eq!(sai.count_objects(SaiObjectType::BufferPool), 0);
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
