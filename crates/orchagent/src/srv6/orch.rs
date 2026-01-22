//! SRv6 orchestration logic.

use super::types::{Srv6LocalSidEntry, Srv6Sid, Srv6SidListEntry, Srv6Stats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Srv6OrchError {
    LocalSidNotFound(Srv6Sid),
    SidListNotFound(String),
    InvalidSid(String),
    InvalidEndpointBehavior(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct Srv6OrchConfig {
    pub enable_my_sid_table: bool,
    pub enable_usp: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Srv6OrchStats {
    pub stats: Srv6Stats,
    pub errors: u64,
}

pub trait Srv6OrchCallbacks: Send + Sync {
    fn on_local_sid_created(&self, entry: &Srv6LocalSidEntry);
    fn on_local_sid_removed(&self, sid: &Srv6Sid);
    fn on_sidlist_created(&self, entry: &Srv6SidListEntry);
    fn on_sidlist_removed(&self, name: &str);
}

pub struct Srv6Orch {
    config: Srv6OrchConfig,
    stats: Srv6OrchStats,
    local_sids: HashMap<Srv6Sid, Srv6LocalSidEntry>,
    sidlists: HashMap<String, Srv6SidListEntry>,
}

impl Srv6Orch {
    pub fn new(config: Srv6OrchConfig) -> Self {
        Self {
            config,
            stats: Srv6OrchStats::default(),
            local_sids: HashMap::new(),
            sidlists: HashMap::new(),
        }
    }

    pub fn get_local_sid(&self, sid: &Srv6Sid) -> Option<&Srv6LocalSidEntry> {
        self.local_sids.get(sid)
    }

    pub fn add_local_sid(&mut self, entry: Srv6LocalSidEntry) -> Result<(), Srv6OrchError> {
        let sid = entry.config.sid.clone();

        if self.local_sids.contains_key(&sid) {
            return Err(Srv6OrchError::SaiError("Local SID already exists".to_string()));
        }

        self.stats.stats.local_sids_created = self.stats.stats.local_sids_created.saturating_add(1);
        self.local_sids.insert(sid, entry);

        Ok(())
    }

    pub fn remove_local_sid(&mut self, sid: &Srv6Sid) -> Result<Srv6LocalSidEntry, Srv6OrchError> {
        self.local_sids.remove(sid)
            .ok_or_else(|| Srv6OrchError::LocalSidNotFound(sid.clone()))
    }

    pub fn get_sidlist(&self, name: &str) -> Option<&Srv6SidListEntry> {
        self.sidlists.get(name)
    }

    pub fn add_sidlist(&mut self, entry: Srv6SidListEntry) -> Result<(), Srv6OrchError> {
        let name = entry.config.name.clone();

        if self.sidlists.contains_key(&name) {
            return Err(Srv6OrchError::SaiError("SID list already exists".to_string()));
        }

        // Validate SIDs in the list
        for sid in &entry.config.sids {
            if let Err(e) = Srv6Sid::from_str(sid.as_str()) {
                return Err(Srv6OrchError::InvalidSid(e));
            }
        }

        self.stats.stats.sidlists_created = self.stats.stats.sidlists_created.saturating_add(1);
        self.sidlists.insert(name, entry);

        Ok(())
    }

    pub fn remove_sidlist(&mut self, name: &str) -> Result<Srv6SidListEntry, Srv6OrchError> {
        self.sidlists.remove(name)
            .ok_or_else(|| Srv6OrchError::SidListNotFound(name.to_string()))
    }

    pub fn get_sidlists_using_sid(&self, sid: &Srv6Sid) -> Vec<&Srv6SidListEntry> {
        self.sidlists
            .values()
            .filter(|entry| entry.config.sids.contains(sid))
            .collect()
    }

    pub fn local_sid_count(&self) -> usize {
        self.local_sids.len()
    }

    pub fn sidlist_count(&self) -> usize {
        self.sidlists.len()
    }

    pub fn stats(&self) -> &Srv6OrchStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{Srv6LocalSidConfig, Srv6SidListConfig, Srv6EndpointBehavior};

    fn create_test_local_sid(sid_str: &str, behavior: Srv6EndpointBehavior) -> Srv6LocalSidEntry {
        Srv6LocalSidEntry::new(Srv6LocalSidConfig {
            sid: Srv6Sid::new(sid_str.to_string()),
            endpoint_behavior: behavior,
            next_hop: None,
            vrf: None,
        })
    }

    fn create_test_sidlist(name: &str, sids: Vec<&str>) -> Srv6SidListEntry {
        let sid_vec: Vec<Srv6Sid> = sids.iter()
            .map(|s| Srv6Sid::new(s.to_string()))
            .collect();

        Srv6SidListEntry::new(Srv6SidListConfig {
            name: name.to_string(),
            sids: sid_vec,
        })
    }

    #[test]
    fn test_add_local_sid() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());
        let sid_entry = create_test_local_sid("fc00:0:1:1::", Srv6EndpointBehavior::End);
        let sid = sid_entry.config.sid.clone();

        assert_eq!(orch.local_sid_count(), 0);
        orch.add_local_sid(sid_entry).unwrap();
        assert_eq!(orch.local_sid_count(), 1);
        assert_eq!(orch.stats().stats.local_sids_created, 1);

        // Verify we can retrieve it
        let retrieved = orch.get_local_sid(&sid);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_add_duplicate_local_sid() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());
        let sid_entry1 = create_test_local_sid("fc00:0:1:1::", Srv6EndpointBehavior::End);
        let sid_entry2 = create_test_local_sid("fc00:0:1:1::", Srv6EndpointBehavior::EndX);

        orch.add_local_sid(sid_entry1).unwrap();
        assert_eq!(orch.local_sid_count(), 1);

        // Adding duplicate should return an error
        let result = orch.add_local_sid(sid_entry2);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Srv6OrchError::SaiError(_)));
        assert_eq!(orch.local_sid_count(), 1);
    }

    #[test]
    fn test_remove_local_sid() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());
        let sid_entry = create_test_local_sid("fc00:0:1:1::", Srv6EndpointBehavior::End);
        let sid = sid_entry.config.sid.clone();

        orch.add_local_sid(sid_entry).unwrap();
        assert_eq!(orch.local_sid_count(), 1);

        let removed = orch.remove_local_sid(&sid);
        assert!(removed.is_ok());
        assert_eq!(orch.local_sid_count(), 0);

        // Verify it's actually gone
        let retrieved = orch.get_local_sid(&sid);
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_remove_local_sid_not_found() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());
        let sid = Srv6Sid::new("fc00:0:1:1::".to_string());

        let result = orch.remove_local_sid(&sid);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Srv6OrchError::LocalSidNotFound(_)));
    }

    #[test]
    fn test_add_sidlist() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());
        let sidlist = create_test_sidlist("policy1", vec!["fc00:0:1:1::", "fc00:0:1:2::"]);

        assert_eq!(orch.sidlist_count(), 0);
        orch.add_sidlist(sidlist).unwrap();
        assert_eq!(orch.sidlist_count(), 1);
        assert_eq!(orch.stats().stats.sidlists_created, 1);

        // Verify we can retrieve it
        let retrieved = orch.get_sidlist("policy1");
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_add_sidlist_invalid_sid() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());

        // Create a SID list with an invalid SID (no colons)
        let invalid_sidlist = Srv6SidListEntry::new(Srv6SidListConfig {
            name: "invalid_policy".to_string(),
            sids: vec![Srv6Sid::new("invalid_sid".to_string())],
        });

        let result = orch.add_sidlist(invalid_sidlist);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Srv6OrchError::InvalidSid(_)));
        assert_eq!(orch.sidlist_count(), 0);
    }

    #[test]
    fn test_remove_sidlist() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());
        let sidlist = create_test_sidlist("policy1", vec!["fc00:0:1:1::", "fc00:0:1:2::"]);

        orch.add_sidlist(sidlist).unwrap();
        assert_eq!(orch.sidlist_count(), 1);

        let removed = orch.remove_sidlist("policy1");
        assert!(removed.is_ok());
        assert_eq!(orch.sidlist_count(), 0);

        // Verify it's actually gone
        let retrieved = orch.get_sidlist("policy1");
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_remove_sidlist_not_found() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());

        let result = orch.remove_sidlist("nonexistent");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Srv6OrchError::SidListNotFound(_)));
    }

    #[test]
    fn test_get_sidlists_using_sid() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());
        let sid1 = Srv6Sid::new("fc00:0:1:1::".to_string());
        let sid2 = Srv6Sid::new("fc00:0:1:2::".to_string());
        let sid3 = Srv6Sid::new("fc00:0:1:3::".to_string());

        // Create multiple SID lists
        let sidlist1 = create_test_sidlist("policy1", vec!["fc00:0:1:1::", "fc00:0:1:2::"]);
        let sidlist2 = create_test_sidlist("policy2", vec!["fc00:0:1:1::", "fc00:0:1:3::"]);
        let sidlist3 = create_test_sidlist("policy3", vec!["fc00:0:1:3::", "fc00:0:1:2::"]);

        orch.add_sidlist(sidlist1).unwrap();
        orch.add_sidlist(sidlist2).unwrap();
        orch.add_sidlist(sidlist3).unwrap();

        // Test filtering by sid1 (should return policy1 and policy2)
        let lists_with_sid1 = orch.get_sidlists_using_sid(&sid1);
        assert_eq!(lists_with_sid1.len(), 2);

        // Test filtering by sid2 (should return policy1 and policy3)
        let lists_with_sid2 = orch.get_sidlists_using_sid(&sid2);
        assert_eq!(lists_with_sid2.len(), 2);

        // Test filtering by sid3 (should return policy2 and policy3)
        let lists_with_sid3 = orch.get_sidlists_using_sid(&sid3);
        assert_eq!(lists_with_sid3.len(), 2);

        // Test with a SID that's not in any list
        let sid4 = Srv6Sid::new("fc00:0:1:4::".to_string());
        let lists_with_sid4 = orch.get_sidlists_using_sid(&sid4);
        assert_eq!(lists_with_sid4.len(), 0);
    }

    #[test]
    fn test_local_sid_count() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());
        assert_eq!(orch.local_sid_count(), 0);

        let sid1 = create_test_local_sid("fc00:0:1:1::", Srv6EndpointBehavior::End);
        orch.add_local_sid(sid1).unwrap();
        assert_eq!(orch.local_sid_count(), 1);

        let sid2 = create_test_local_sid("fc00:0:1:2::", Srv6EndpointBehavior::EndX);
        orch.add_local_sid(sid2).unwrap();
        assert_eq!(orch.local_sid_count(), 2);

        let sid3 = create_test_local_sid("fc00:0:1:3::", Srv6EndpointBehavior::EndT);
        orch.add_local_sid(sid3).unwrap();
        assert_eq!(orch.local_sid_count(), 3);

        // Remove one and verify count decreases
        let sid = Srv6Sid::new("fc00:0:1:2::".to_string());
        orch.remove_local_sid(&sid).unwrap();
        assert_eq!(orch.local_sid_count(), 2);
    }

    #[test]
    fn test_sidlist_count() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());
        assert_eq!(orch.sidlist_count(), 0);

        let sidlist1 = create_test_sidlist("policy1", vec!["fc00:0:1:1::", "fc00:0:1:2::"]);
        orch.add_sidlist(sidlist1).unwrap();
        assert_eq!(orch.sidlist_count(), 1);

        let sidlist2 = create_test_sidlist("policy2", vec!["fc00:0:1:3::", "fc00:0:1:4::"]);
        orch.add_sidlist(sidlist2).unwrap();
        assert_eq!(orch.sidlist_count(), 2);

        let sidlist3 = create_test_sidlist("policy3", vec!["fc00:0:1:5::", "fc00:0:1:6::"]);
        orch.add_sidlist(sidlist3).unwrap();
        assert_eq!(orch.sidlist_count(), 3);

        // Remove one and verify count decreases
        orch.remove_sidlist("policy2").unwrap();
        assert_eq!(orch.sidlist_count(), 2);
    }

    #[test]
    fn test_add_duplicate_sidlist() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());
        let sidlist1 = create_test_sidlist("policy1", vec!["fc00:0:1:1::", "fc00:0:1:2::"]);
        let sidlist2 = create_test_sidlist("policy1", vec!["fc00:0:1:3::", "fc00:0:1:4::"]);

        orch.add_sidlist(sidlist1).unwrap();
        assert_eq!(orch.sidlist_count(), 1);

        // Adding duplicate should return an error
        let result = orch.add_sidlist(sidlist2);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Srv6OrchError::SaiError(_)));
        assert_eq!(orch.sidlist_count(), 1);
    }

    #[test]
    fn test_local_sid_stats() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());
        assert_eq!(orch.stats().stats.local_sids_created, 0);

        let sid1 = create_test_local_sid("fc00:0:1:1::", Srv6EndpointBehavior::End);
        orch.add_local_sid(sid1).unwrap();
        assert_eq!(orch.stats().stats.local_sids_created, 1);

        let sid2 = create_test_local_sid("fc00:0:1:2::", Srv6EndpointBehavior::EndX);
        orch.add_local_sid(sid2).unwrap();
        assert_eq!(orch.stats().stats.local_sids_created, 2);

        let sid3 = create_test_local_sid("fc00:0:1:3::", Srv6EndpointBehavior::EndT);
        orch.add_local_sid(sid3).unwrap();
        assert_eq!(orch.stats().stats.local_sids_created, 3);
    }

    #[test]
    fn test_sidlist_stats() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());
        assert_eq!(orch.stats().stats.sidlists_created, 0);

        let sidlist1 = create_test_sidlist("policy1", vec!["fc00:0:1:1::", "fc00:0:1:2::"]);
        orch.add_sidlist(sidlist1).unwrap();
        assert_eq!(orch.stats().stats.sidlists_created, 1);

        let sidlist2 = create_test_sidlist("policy2", vec!["fc00:0:1:3::", "fc00:0:1:4::"]);
        orch.add_sidlist(sidlist2).unwrap();
        assert_eq!(orch.stats().stats.sidlists_created, 2);
    }

    #[test]
    fn test_multiple_local_sids_different_behaviors() {
        let mut orch = Srv6Orch::new(Srv6OrchConfig::default());

        orch.add_local_sid(create_test_local_sid("fc00:0:1:1::", Srv6EndpointBehavior::End)).unwrap();
        orch.add_local_sid(create_test_local_sid("fc00:0:1:2::", Srv6EndpointBehavior::EndX)).unwrap();
        orch.add_local_sid(create_test_local_sid("fc00:0:1:3::", Srv6EndpointBehavior::EndDx6)).unwrap();
        orch.add_local_sid(create_test_local_sid("fc00:0:1:4::", Srv6EndpointBehavior::EndDt4)).unwrap();

        assert_eq!(orch.local_sid_count(), 4);

        // Verify each SID can be retrieved with correct behavior
        let sid1 = orch.get_local_sid(&Srv6Sid::new("fc00:0:1:1::".to_string())).unwrap();
        assert!(matches!(sid1.config.endpoint_behavior, Srv6EndpointBehavior::End));

        let sid2 = orch.get_local_sid(&Srv6Sid::new("fc00:0:1:2::".to_string())).unwrap();
        assert!(matches!(sid2.config.endpoint_behavior, Srv6EndpointBehavior::EndX));

        let sid3 = orch.get_local_sid(&Srv6Sid::new("fc00:0:1:3::".to_string())).unwrap();
        assert!(matches!(sid3.config.endpoint_behavior, Srv6EndpointBehavior::EndDx6));

        let sid4 = orch.get_local_sid(&Srv6Sid::new("fc00:0:1:4::".to_string())).unwrap();
        assert!(matches!(sid4.config.endpoint_behavior, Srv6EndpointBehavior::EndDt4));
    }
}
