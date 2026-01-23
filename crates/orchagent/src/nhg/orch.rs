//! Next hop group orchestration logic.

use super::types::{LabelStack, NextHopGroupMember, NextHopKey};
use crate::{audit_log, audit::{AuditCategory, AuditOutcome, AuditRecord}};
use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum NhgOrchError {
    #[error("NHG already exists: {0}")]
    NhgExists(String),
    #[error("NHG not found: {0}")]
    NhgNotFound(String),
    #[error("Next hop not found: {0}")]
    NextHopNotFound(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("SAI error: {0}")]
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct NhgOrchConfig {
    pub max_nhgs: u32,
}

#[derive(Debug, Clone, Default)]
pub struct NhgOrchStats {
    pub nhgs_created: u64,
    pub nhgs_removed: u64,
    pub nexthops_created: u64,
    pub nexthops_removed: u64,
}

pub trait NhgOrchCallbacks: Send + Sync {
    fn create_next_hop(&self, key: &NextHopKey) -> Result<RawSaiObjectId, String>;
    fn remove_next_hop(&self, nh_id: RawSaiObjectId) -> Result<(), String>;
    fn create_next_hop_group(&self, members: &[NextHopGroupMember]) -> Result<RawSaiObjectId, String>;
    fn remove_next_hop_group(&self, nhg_id: RawSaiObjectId) -> Result<(), String>;
}

#[derive(Debug)]
pub struct NhgOrchEntry {
    pub name: String,
    pub nhg_id: RawSaiObjectId,
    pub members: Vec<NextHopGroupMember>,
    pub ref_count: AtomicU32,
}

pub struct NhgOrch {
    config: NhgOrchConfig,
    stats: NhgOrchStats,
    callbacks: Option<Arc<dyn NhgOrchCallbacks>>,
    nhgs: HashMap<String, NhgOrchEntry>,
    nexthops: HashMap<NextHopKey, RawSaiObjectId>,
}

impl NhgOrch {
    pub fn new(config: NhgOrchConfig) -> Self {
        Self {
            config,
            stats: NhgOrchStats::default(),
            callbacks: None,
            nhgs: HashMap::new(),
            nexthops: HashMap::new(),
        }
    }

    pub fn set_callbacks(&mut self, callbacks: Arc<dyn NhgOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    pub fn nhg_exists(&self, name: &str) -> bool {
        self.nhgs.contains_key(name)
    }

    pub fn nhg_count(&self) -> usize {
        self.nhgs.len()
    }

    pub fn nexthop_count(&self) -> usize {
        self.nexthops.len()
    }

    pub fn stats(&self) -> &NhgOrchStats {
        &self.stats
    }

    pub fn get_or_create_nexthop(&mut self, key: NextHopKey) -> Result<RawSaiObjectId, NhgOrchError> {
        if let Some(&oid) = self.nexthops.get(&key) {
            return Ok(oid);
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| NhgOrchError::InvalidConfig("No callbacks set".to_string()))?,
        );

        let nh_id = callbacks.create_next_hop(&key)
            .map_err(NhgOrchError::SaiError)?;

        self.nexthops.insert(key, nh_id);
        self.stats.nexthops_created += 1;

        Ok(nh_id)
    }

    pub fn create_nhg(&mut self, name: String, members: Vec<NextHopGroupMember>) -> Result<(), NhgOrchError> {
        if self.nhgs.contains_key(&name) {
            let err = NhgOrchError::NhgExists(name.clone());
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceCreate, "NhgOrch", "create_nhg")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(name)
                    .with_object_type("next_hop_group")
                    .with_error(err.to_string())
            );
            return Err(err);
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| NhgOrchError::InvalidConfig("No callbacks set".to_string()))?,
        );

        let nhg_id = match callbacks.create_next_hop_group(&members) {
            Ok(id) => id,
            Err(e) => {
                let err = NhgOrchError::SaiError(e);
                audit_log!(
                    AuditRecord::new(AuditCategory::ResourceCreate, "NhgOrch", "create_nhg")
                        .with_outcome(AuditOutcome::Failure)
                        .with_object_id(name)
                        .with_object_type("next_hop_group")
                        .with_error(err.to_string())
                        .with_details(serde_json::json!({
                            "member_count": members.len(),
                        }))
                );
                return Err(err);
            }
        };

        let entry = NhgOrchEntry {
            name: name.clone(),
            nhg_id,
            members: members.clone(),
            ref_count: AtomicU32::new(0),
        };

        self.nhgs.insert(name.clone(), entry);
        self.stats.nhgs_created += 1;

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceCreate, "NhgOrch", "create_nhg")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(name)
                .with_object_type("next_hop_group")
                .with_details(serde_json::json!({
                    "member_count": members.len(),
                    "nhg_id": format!("{:#x}", nhg_id),
                    "ref_count": 0,
                }))
        );

        Ok(())
    }

    pub fn remove_nhg(&mut self, name: &str) -> Result<(), NhgOrchError> {
        let entry = self.nhgs.get(name)
            .ok_or_else(|| NhgOrchError::NhgNotFound(name.to_string()))?;

        let ref_count = entry.ref_count.load(Ordering::SeqCst);
        if ref_count > 0 {
            let err = NhgOrchError::InvalidConfig(
                format!("NHG {} still in use (ref_count={})", name, ref_count)
            );
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceDelete, "NhgOrch", "remove_nhg")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(name)
                    .with_object_type("next_hop_group")
                    .with_error(err.to_string())
                    .with_details(serde_json::json!({
                        "ref_count": ref_count,
                    }))
            );
            return Err(err);
        }

        let entry = self.nhgs.remove(name).unwrap();

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| NhgOrchError::InvalidConfig("No callbacks set".to_string()))?;

        if let Err(e) = callbacks.remove_next_hop_group(entry.nhg_id) {
            let err = NhgOrchError::SaiError(e);
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceDelete, "NhgOrch", "remove_nhg")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(name)
                    .with_object_type("next_hop_group")
                    .with_error(err.to_string())
            );
            return Err(err);
        }

        self.stats.nhgs_removed += 1;

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceDelete, "NhgOrch", "remove_nhg")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(name)
                .with_object_type("next_hop_group")
                .with_details(serde_json::json!({
                    "member_count": entry.members.len(),
                    "nhg_id": format!("{:#x}", entry.nhg_id),
                }))
        );

        Ok(())
    }

    pub fn increment_nhg_ref(&self, name: &str) -> Result<u32, NhgOrchError> {
        let entry = self.nhgs.get(name)
            .ok_or_else(|| NhgOrchError::NhgNotFound(name.to_string()))?;

        let prev = entry.ref_count.fetch_add(1, Ordering::SeqCst);
        let new_count = prev.saturating_add(1);

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceModify, "NhgOrch", "add_member")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(name)
                .with_object_type("next_hop_group_reference")
                .with_details(serde_json::json!({
                    "ref_count_before": prev,
                    "ref_count_after": new_count,
                }))
        );

        Ok(new_count)
    }

    pub fn decrement_nhg_ref(&self, name: &str) -> Result<u32, NhgOrchError> {
        let entry = self.nhgs.get(name)
            .ok_or_else(|| NhgOrchError::NhgNotFound(name.to_string()))?;

        let prev = entry.ref_count.load(Ordering::SeqCst);
        if prev == 0 {
            let err = NhgOrchError::InvalidConfig(
                format!("NHG {} ref_count already at 0", name)
            );
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceModify, "NhgOrch", "remove_member")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(name)
                    .with_object_type("next_hop_group_reference")
                    .with_error(err.to_string())
            );
            return Err(err);
        }

        entry.ref_count.fetch_sub(1, Ordering::SeqCst);
        let new_count = prev - 1;

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceModify, "NhgOrch", "remove_member")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(name)
                .with_object_type("next_hop_group_reference")
                .with_details(serde_json::json!({
                    "ref_count_before": prev,
                    "ref_count_after": new_count,
                }))
        );

        Ok(new_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonic_types::{IpAddress, MacAddress};
    use std::str::FromStr;
    use std::sync::atomic::AtomicU64;

    struct MockCallbacks {
        next_nhg_id: AtomicU64,
        next_nh_id: AtomicU64,
    }

    impl MockCallbacks {
        fn new() -> Self {
            Self {
                next_nhg_id: AtomicU64::new(0x4000),
                next_nh_id: AtomicU64::new(0x3000),
            }
        }
    }

    impl NhgOrchCallbacks for MockCallbacks {
        fn create_next_hop(&self, _key: &NextHopKey) -> Result<RawSaiObjectId, String> {
            Ok(self.next_nh_id.fetch_add(1, Ordering::SeqCst))
        }
        fn remove_next_hop(&self, _nh_id: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }
        fn create_next_hop_group(&self, _members: &[NextHopGroupMember]) -> Result<RawSaiObjectId, String> {
            Ok(self.next_nhg_id.fetch_add(1, Ordering::SeqCst))
        }
        fn remove_next_hop_group(&self, _nhg_id: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }
    }

    struct FailingCallbacks;
    impl NhgOrchCallbacks for FailingCallbacks {
        fn create_next_hop(&self, _key: &NextHopKey) -> Result<RawSaiObjectId, String> {
            Err("Failed to create next hop".to_string())
        }
        fn remove_next_hop(&self, _nh_id: RawSaiObjectId) -> Result<(), String> {
            Err("Failed to remove next hop".to_string())
        }
        fn create_next_hop_group(&self, _members: &[NextHopGroupMember]) -> Result<RawSaiObjectId, String> {
            Err("Failed to create NHG".to_string())
        }
        fn remove_next_hop_group(&self, _nhg_id: RawSaiObjectId) -> Result<(), String> {
            Err("Failed to remove NHG".to_string())
        }
    }

    fn create_test_nexthop_key(ip: &str, alias: &str) -> NextHopKey {
        NextHopKey {
            ip_address: IpAddress::from_str(ip).unwrap(),
            alias: alias.to_string(),
            vni: 0,
            mac_address: None,
            label_stack: LabelStack::default(),
            weight: 0,
            srv6_segment: None,
            srv6_source: None,
            srv6_vpn_sid: None,
        }
    }

    fn create_test_member(ip: &str, alias: &str) -> NextHopGroupMember {
        NextHopGroupMember {
            key: create_test_nexthop_key(ip, alias),
            gm_id: 0,
            nh_id: 0,
        }
    }

    fn create_weighted_member(ip: &str, alias: &str, weight: u32) -> NextHopGroupMember {
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

    // 1. Next-Hop Group Management Tests

    #[test]
    fn test_create_nhg_single_nexthop() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = create_test_member("10.0.0.1", "Ethernet0");
        assert!(orch.create_nhg("nhg1".to_string(), vec![member]).is_ok());
        assert_eq!(orch.nhg_count(), 1);
        assert!(orch.nhg_exists("nhg1"));
        assert_eq!(orch.stats().nhgs_created, 1);
    }

    #[test]
    fn test_create_nhg_multiple_nexthops_ecmp() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let members = vec![
            create_test_member("10.0.0.1", "Ethernet0"),
            create_test_member("10.0.0.2", "Ethernet4"),
            create_test_member("10.0.0.3", "Ethernet8"),
            create_test_member("10.0.0.4", "Ethernet12"),
        ];

        assert!(orch.create_nhg("ecmp_group".to_string(), members).is_ok());
        assert_eq!(orch.nhg_count(), 1);
        assert!(orch.nhg_exists("ecmp_group"));
    }

    #[test]
    fn test_create_nhg_weighted_wcmp() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let members = vec![
            create_weighted_member("10.0.0.1", "Ethernet0", 100),
            create_weighted_member("10.0.0.2", "Ethernet4", 200),
            create_weighted_member("10.0.0.3", "Ethernet8", 50),
        ];

        assert!(orch.create_nhg("wcmp_group".to_string(), members).is_ok());
        assert!(orch.nhg_exists("wcmp_group"));
    }

    #[test]
    fn test_create_duplicate_nhg() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = create_test_member("10.0.0.1", "Ethernet0");
        assert!(orch.create_nhg("nhg1".to_string(), vec![member.clone()]).is_ok());

        let result = orch.create_nhg("nhg1".to_string(), vec![member]);
        assert!(matches!(result, Err(NhgOrchError::NhgExists(_))));
        assert_eq!(orch.nhg_count(), 1);
    }

    #[test]
    fn test_remove_nhg() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = create_test_member("10.0.0.1", "Ethernet0");
        orch.create_nhg("nhg1".to_string(), vec![member]).unwrap();
        assert_eq!(orch.nhg_count(), 1);

        assert!(orch.remove_nhg("nhg1").is_ok());
        assert_eq!(orch.nhg_count(), 0);
        assert!(!orch.nhg_exists("nhg1"));
        assert_eq!(orch.stats().nhgs_removed, 1);
    }

    #[test]
    fn test_remove_nonexistent_nhg() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let result = orch.remove_nhg("nonexistent");
        assert!(matches!(result, Err(NhgOrchError::NhgNotFound(_))));
    }

    // 2. Next-Hop Operations Tests

    #[test]
    fn test_create_nexthop() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let nh_key = create_test_nexthop_key("10.0.0.1", "Ethernet0");
        let result = orch.get_or_create_nexthop(nh_key.clone());
        assert!(result.is_ok());
        assert_eq!(orch.nexthop_count(), 1);
        assert_eq!(orch.stats().nexthops_created, 1);
    }

    #[test]
    fn test_reuse_existing_nexthop() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let nh_key = create_test_nexthop_key("10.0.0.1", "Ethernet0");
        let oid1 = orch.get_or_create_nexthop(nh_key.clone()).unwrap();
        let oid2 = orch.get_or_create_nexthop(nh_key).unwrap();

        assert_eq!(oid1, oid2);
        assert_eq!(orch.nexthop_count(), 1);
        assert_eq!(orch.stats().nexthops_created, 1);
    }

    #[test]
    fn test_create_multiple_nexthops() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let nh1 = create_test_nexthop_key("10.0.0.1", "Ethernet0");
        let nh2 = create_test_nexthop_key("10.0.0.2", "Ethernet4");
        let nh3 = create_test_nexthop_key("10.0.0.3", "Ethernet8");

        orch.get_or_create_nexthop(nh1).unwrap();
        orch.get_or_create_nexthop(nh2).unwrap();
        orch.get_or_create_nexthop(nh3).unwrap();

        assert_eq!(orch.nexthop_count(), 3);
        assert_eq!(orch.stats().nexthops_created, 3);
    }

    #[test]
    fn test_create_nexthop_without_callbacks() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());

        let nh_key = create_test_nexthop_key("10.0.0.1", "Ethernet0");
        let result = orch.get_or_create_nexthop(nh_key);
        assert!(matches!(result, Err(NhgOrchError::InvalidConfig(_))));
    }

    // 3. Group Types Tests

    #[test]
    fn test_create_overlay_nhg() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = NextHopGroupMember {
            key: NextHopKey {
                ip_address: IpAddress::from_str("192.168.1.1").unwrap(),
                alias: "Vxlan100".to_string(),
                vni: 1000,
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

        assert!(orch.create_nhg("overlay_nhg".to_string(), vec![member]).is_ok());
        assert!(orch.nhg_exists("overlay_nhg"));
    }

    #[test]
    fn test_create_srv6_nhg() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = NextHopGroupMember {
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

        assert!(orch.create_nhg("srv6_nhg".to_string(), vec![member]).is_ok());
        assert!(orch.nhg_exists("srv6_nhg"));
    }

    #[test]
    fn test_create_srv6_vpn_nhg() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = NextHopGroupMember {
            key: NextHopKey {
                ip_address: IpAddress::from_str("2001:db8::2").unwrap(),
                alias: "Ethernet4".to_string(),
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

        assert!(orch.create_nhg("srv6_vpn_nhg".to_string(), vec![member]).is_ok());
        assert!(orch.nhg_exists("srv6_vpn_nhg"));
    }

    #[test]
    fn test_create_mpls_nhg() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = NextHopGroupMember {
            key: NextHopKey {
                ip_address: IpAddress::from_str("10.0.0.1").unwrap(),
                alias: "Ethernet0".to_string(),
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

        assert!(orch.create_nhg("mpls_nhg".to_string(), vec![member]).is_ok());
        assert!(orch.nhg_exists("mpls_nhg"));
    }

    // 4. Member Management Tests

    #[test]
    fn test_nhg_with_no_members() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        assert!(orch.create_nhg("empty_nhg".to_string(), vec![]).is_ok());
        assert!(orch.nhg_exists("empty_nhg"));
    }

    #[test]
    fn test_nhg_with_single_member() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = create_test_member("10.0.0.1", "Ethernet0");
        assert!(orch.create_nhg("single_member".to_string(), vec![member]).is_ok());
        assert!(orch.nhg_exists("single_member"));
    }

    #[test]
    fn test_nhg_with_many_members() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let members: Vec<_> = (1..=32)
            .map(|i| create_test_member(&format!("10.0.0.{}", i), &format!("Ethernet{}", i * 4)))
            .collect();

        assert!(orch.create_nhg("large_nhg".to_string(), members).is_ok());
        assert!(orch.nhg_exists("large_nhg"));
    }

    #[test]
    fn test_member_sync_state() {
        let member = create_test_member("10.0.0.1", "Ethernet0");
        assert!(!member.is_synced());

        let synced_member = NextHopGroupMember {
            key: create_test_nexthop_key("10.0.0.1", "Ethernet0"),
            gm_id: 0x5000,
            nh_id: 0x3000,
        };
        assert!(synced_member.is_synced());
    }

    // 5. Reference Counting Tests

    #[test]
    fn test_increment_nhg_ref() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = create_test_member("10.0.0.1", "Ethernet0");
        orch.create_nhg("nhg1".to_string(), vec![member]).unwrap();

        assert_eq!(orch.increment_nhg_ref("nhg1").unwrap(), 1);
        assert_eq!(orch.increment_nhg_ref("nhg1").unwrap(), 2);
        assert_eq!(orch.increment_nhg_ref("nhg1").unwrap(), 3);
    }

    #[test]
    fn test_decrement_nhg_ref() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = create_test_member("10.0.0.1", "Ethernet0");
        orch.create_nhg("nhg1".to_string(), vec![member]).unwrap();

        orch.increment_nhg_ref("nhg1").unwrap();
        orch.increment_nhg_ref("nhg1").unwrap();

        assert_eq!(orch.decrement_nhg_ref("nhg1").unwrap(), 1);
        assert_eq!(orch.decrement_nhg_ref("nhg1").unwrap(), 0);
    }

    #[test]
    fn test_cannot_remove_nhg_with_references() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = create_test_member("10.0.0.1", "Ethernet0");
        orch.create_nhg("nhg1".to_string(), vec![member]).unwrap();

        orch.increment_nhg_ref("nhg1").unwrap();

        let result = orch.remove_nhg("nhg1");
        assert!(matches!(result, Err(NhgOrchError::InvalidConfig(_))));
        assert!(orch.nhg_exists("nhg1"));
    }

    #[test]
    fn test_remove_nhg_after_ref_count_zero() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = create_test_member("10.0.0.1", "Ethernet0");
        orch.create_nhg("nhg1".to_string(), vec![member]).unwrap();

        orch.increment_nhg_ref("nhg1").unwrap();
        orch.decrement_nhg_ref("nhg1").unwrap();

        assert!(orch.remove_nhg("nhg1").is_ok());
        assert!(!orch.nhg_exists("nhg1"));
    }

    #[test]
    fn test_decrement_ref_count_already_zero() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = create_test_member("10.0.0.1", "Ethernet0");
        orch.create_nhg("nhg1".to_string(), vec![member]).unwrap();

        let result = orch.decrement_nhg_ref("nhg1");
        assert!(matches!(result, Err(NhgOrchError::InvalidConfig(_))));
    }

    #[test]
    fn test_increment_ref_nonexistent_nhg() {
        let orch = NhgOrch::new(NhgOrchConfig::default());
        let result = orch.increment_nhg_ref("nonexistent");
        assert!(matches!(result, Err(NhgOrchError::NhgNotFound(_))));
    }

    #[test]
    fn test_decrement_ref_nonexistent_nhg() {
        let orch = NhgOrch::new(NhgOrchConfig::default());
        let result = orch.decrement_nhg_ref("nonexistent");
        assert!(matches!(result, Err(NhgOrchError::NhgNotFound(_))));
    }

    #[test]
    fn test_multiple_routes_reference_same_nhg() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = create_test_member("10.0.0.1", "Ethernet0");
        orch.create_nhg("nhg1".to_string(), vec![member]).unwrap();

        // Simulate 5 routes referencing the same NHG
        for _ in 0..5 {
            orch.increment_nhg_ref("nhg1").unwrap();
        }

        // Cannot remove while references exist
        assert!(orch.remove_nhg("nhg1").is_err());

        // Remove all references
        for _ in 0..5 {
            orch.decrement_nhg_ref("nhg1").unwrap();
        }

        // Now can remove
        assert!(orch.remove_nhg("nhg1").is_ok());
    }

    // 6. Error Handling Tests

    #[test]
    fn test_create_nhg_sai_error() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(FailingCallbacks));

        let member = create_test_member("10.0.0.1", "Ethernet0");
        let result = orch.create_nhg("nhg1".to_string(), vec![member]);
        assert!(matches!(result, Err(NhgOrchError::SaiError(_))));
        assert_eq!(orch.nhg_count(), 0);
    }

    #[test]
    fn test_remove_nhg_sai_error() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let member = create_test_member("10.0.0.1", "Ethernet0");
        orch.create_nhg("nhg1".to_string(), vec![member]).unwrap();

        // Replace with failing callbacks
        orch.set_callbacks(Arc::new(FailingCallbacks));

        let result = orch.remove_nhg("nhg1");
        assert!(matches!(result, Err(NhgOrchError::SaiError(_))));
    }

    #[test]
    fn test_create_nexthop_sai_error() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(FailingCallbacks));

        let nh_key = create_test_nexthop_key("10.0.0.1", "Ethernet0");
        let result = orch.get_or_create_nexthop(nh_key);
        assert!(matches!(result, Err(NhgOrchError::SaiError(_))));
        assert_eq!(orch.nexthop_count(), 0);
    }

    // 7. Statistics Tests

    #[test]
    fn test_nhg_count_tracking() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        assert_eq!(orch.nhg_count(), 0);

        let member = create_test_member("10.0.0.1", "Ethernet0");
        orch.create_nhg("nhg1".to_string(), vec![member.clone()]).unwrap();
        assert_eq!(orch.nhg_count(), 1);

        orch.create_nhg("nhg2".to_string(), vec![member.clone()]).unwrap();
        assert_eq!(orch.nhg_count(), 2);

        orch.create_nhg("nhg3".to_string(), vec![member]).unwrap();
        assert_eq!(orch.nhg_count(), 3);

        orch.remove_nhg("nhg2").unwrap();
        assert_eq!(orch.nhg_count(), 2);
    }

    #[test]
    fn test_nexthop_count_tracking() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        assert_eq!(orch.nexthop_count(), 0);

        orch.get_or_create_nexthop(create_test_nexthop_key("10.0.0.1", "Ethernet0")).unwrap();
        assert_eq!(orch.nexthop_count(), 1);

        orch.get_or_create_nexthop(create_test_nexthop_key("10.0.0.2", "Ethernet4")).unwrap();
        assert_eq!(orch.nexthop_count(), 2);

        // Reusing existing nexthop doesn't increase count
        orch.get_or_create_nexthop(create_test_nexthop_key("10.0.0.1", "Ethernet0")).unwrap();
        assert_eq!(orch.nexthop_count(), 2);
    }

    #[test]
    fn test_stats_nhgs_created() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        assert_eq!(orch.stats().nhgs_created, 0);

        let member = create_test_member("10.0.0.1", "Ethernet0");
        orch.create_nhg("nhg1".to_string(), vec![member.clone()]).unwrap();
        assert_eq!(orch.stats().nhgs_created, 1);

        orch.create_nhg("nhg2".to_string(), vec![member]).unwrap();
        assert_eq!(orch.stats().nhgs_created, 2);
    }

    #[test]
    fn test_stats_nhgs_removed() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        assert_eq!(orch.stats().nhgs_removed, 0);

        let member = create_test_member("10.0.0.1", "Ethernet0");
        orch.create_nhg("nhg1".to_string(), vec![member.clone()]).unwrap();
        orch.create_nhg("nhg2".to_string(), vec![member]).unwrap();

        orch.remove_nhg("nhg1").unwrap();
        assert_eq!(orch.stats().nhgs_removed, 1);

        orch.remove_nhg("nhg2").unwrap();
        assert_eq!(orch.stats().nhgs_removed, 2);
    }

    #[test]
    fn test_stats_nexthops_created() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        assert_eq!(orch.stats().nexthops_created, 0);

        orch.get_or_create_nexthop(create_test_nexthop_key("10.0.0.1", "Ethernet0")).unwrap();
        assert_eq!(orch.stats().nexthops_created, 1);

        orch.get_or_create_nexthop(create_test_nexthop_key("10.0.0.2", "Ethernet4")).unwrap();
        assert_eq!(orch.stats().nexthops_created, 2);

        // Reusing doesn't increment
        orch.get_or_create_nexthop(create_test_nexthop_key("10.0.0.1", "Ethernet0")).unwrap();
        assert_eq!(orch.stats().nexthops_created, 2);
    }

    // 8. Edge Cases

    #[test]
    fn test_create_multiple_nhgs_same_members() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let members = vec![
            create_test_member("10.0.0.1", "Ethernet0"),
            create_test_member("10.0.0.2", "Ethernet4"),
        ];

        orch.create_nhg("nhg1".to_string(), members.clone()).unwrap();
        orch.create_nhg("nhg2".to_string(), members).unwrap();

        assert_eq!(orch.nhg_count(), 2);
        assert!(orch.nhg_exists("nhg1"));
        assert!(orch.nhg_exists("nhg2"));
    }

    #[test]
    fn test_nhg_exists_check() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        assert!(!orch.nhg_exists("nhg1"));

        let member = create_test_member("10.0.0.1", "Ethernet0");
        orch.create_nhg("nhg1".to_string(), vec![member]).unwrap();

        assert!(orch.nhg_exists("nhg1"));
        assert!(!orch.nhg_exists("nhg2"));

        orch.remove_nhg("nhg1").unwrap();
        assert!(!orch.nhg_exists("nhg1"));
    }

    #[test]
    fn test_ipv6_nexthops() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks::new()));

        let members = vec![
            NextHopGroupMember {
                key: NextHopKey {
                    ip_address: IpAddress::from_str("2001:db8::1").unwrap(),
                    alias: "Ethernet0".to_string(),
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
            },
            NextHopGroupMember {
                key: NextHopKey {
                    ip_address: IpAddress::from_str("2001:db8::2").unwrap(),
                    alias: "Ethernet4".to_string(),
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
            },
        ];

        assert!(orch.create_nhg("ipv6_nhg".to_string(), members).is_ok());
        assert!(orch.nhg_exists("ipv6_nhg"));
    }
}
