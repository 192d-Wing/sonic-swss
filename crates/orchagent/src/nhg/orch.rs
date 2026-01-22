//! Next hop group orchestration logic.

use super::types::{LabelStack, NextHopGroupMember, NextHopKey};
use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Clone)]
pub enum NhgOrchError {
    NhgExists(String),
    NhgNotFound(String),
    NextHopNotFound(String),
    InvalidConfig(String),
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
            return Err(NhgOrchError::NhgExists(name));
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| NhgOrchError::InvalidConfig("No callbacks set".to_string()))?,
        );

        let nhg_id = callbacks.create_next_hop_group(&members)
            .map_err(NhgOrchError::SaiError)?;

        let entry = NhgOrchEntry {
            name: name.clone(),
            nhg_id,
            members: members.clone(),
            ref_count: AtomicU32::new(0),
        };

        self.nhgs.insert(name, entry);
        self.stats.nhgs_created += 1;

        Ok(())
    }

    pub fn remove_nhg(&mut self, name: &str) -> Result<(), NhgOrchError> {
        let entry = self.nhgs.get(name)
            .ok_or_else(|| NhgOrchError::NhgNotFound(name.to_string()))?;

        if entry.ref_count.load(Ordering::SeqCst) > 0 {
            return Err(NhgOrchError::InvalidConfig(
                format!("NHG {} still in use (ref_count={})", name, entry.ref_count.load(Ordering::SeqCst))
            ));
        }

        let entry = self.nhgs.remove(name).unwrap();

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| NhgOrchError::InvalidConfig("No callbacks set".to_string()))?;

        callbacks.remove_next_hop_group(entry.nhg_id)
            .map_err(NhgOrchError::SaiError)?;

        self.stats.nhgs_removed += 1;

        Ok(())
    }

    pub fn increment_nhg_ref(&self, name: &str) -> Result<u32, NhgOrchError> {
        let entry = self.nhgs.get(name)
            .ok_or_else(|| NhgOrchError::NhgNotFound(name.to_string()))?;

        let prev = entry.ref_count.fetch_add(1, Ordering::SeqCst);
        Ok(prev.saturating_add(1))
    }

    pub fn decrement_nhg_ref(&self, name: &str) -> Result<u32, NhgOrchError> {
        let entry = self.nhgs.get(name)
            .ok_or_else(|| NhgOrchError::NhgNotFound(name.to_string()))?;

        let prev = entry.ref_count.load(Ordering::SeqCst);
        if prev == 0 {
            return Err(NhgOrchError::InvalidConfig(
                format!("NHG {} ref_count already at 0", name)
            ));
        }

        entry.ref_count.fetch_sub(1, Ordering::SeqCst);
        Ok(prev - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonic_types::IpAddress;
    use std::str::FromStr;

    struct MockCallbacks;
    impl NhgOrchCallbacks for MockCallbacks {
        fn create_next_hop(&self, _key: &NextHopKey) -> Result<RawSaiObjectId, String> {
            Ok(0x3000)
        }
        fn remove_next_hop(&self, _nh_id: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }
        fn create_next_hop_group(&self, _members: &[NextHopGroupMember]) -> Result<RawSaiObjectId, String> {
            Ok(0x4000)
        }
        fn remove_next_hop_group(&self, _nhg_id: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_create_nhg() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let member = NextHopGroupMember {
            key: NextHopKey {
                ip_address: IpAddress::from_str("10.0.0.1").unwrap(),
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
        };

        assert!(orch.create_nhg("nhg1".to_string(), vec![member]).is_ok());
        assert_eq!(orch.nhg_count(), 1);
    }

    #[test]
    fn test_ref_counting() {
        let mut orch = NhgOrch::new(NhgOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let member = NextHopGroupMember {
            key: NextHopKey {
                ip_address: IpAddress::from_str("10.0.0.1").unwrap(),
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
        };

        orch.create_nhg("nhg1".to_string(), vec![member]).unwrap();

        assert_eq!(orch.increment_nhg_ref("nhg1").unwrap(), 1);
        assert_eq!(orch.decrement_nhg_ref("nhg1").unwrap(), 0);
        assert!(orch.decrement_nhg_ref("nhg1").is_err());
    }
}
