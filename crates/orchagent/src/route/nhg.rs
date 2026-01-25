//! Next-hop group types and management.
//!
//! This module provides type-safe next-hop group management with proper
//! reference counting. The key safety improvement is using `SyncMap`
//! to prevent auto-vivification bugs.

use sonic_sai::types::RawSaiObjectId;
use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};

use super::nexthop::NextHopKey;

/// A key identifying a next-hop group (set of next-hops for ECMP).
///
/// The key is the sorted set of next-hop keys, ensuring that two groups
/// with the same next-hops (in any order) have the same key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextHopGroupKey {
    /// The set of next-hops in this group (sorted for deterministic ordering).
    nexthops: BTreeSet<NextHopKey>,
}

impl NextHopGroupKey {
    /// Creates a new empty next-hop group key.
    pub fn new() -> Self {
        Self {
            nexthops: BTreeSet::new(),
        }
    }

    /// Creates a next-hop group key from a single next-hop.
    pub fn single(nexthop: NextHopKey) -> Self {
        let mut nexthops = BTreeSet::new();
        nexthops.insert(nexthop);
        Self { nexthops }
    }

    /// Creates a next-hop group key from multiple next-hops.
    pub fn from_nexthops(nexthops: impl IntoIterator<Item = NextHopKey>) -> Self {
        Self {
            nexthops: nexthops.into_iter().collect(),
        }
    }

    /// Adds a next-hop to the group.
    pub fn add(&mut self, nexthop: NextHopKey) {
        self.nexthops.insert(nexthop);
    }

    /// Removes a next-hop from the group.
    pub fn remove(&mut self, nexthop: &NextHopKey) -> bool {
        self.nexthops.remove(nexthop)
    }

    /// Returns true if the group contains the given next-hop.
    pub fn contains(&self, nexthop: &NextHopKey) -> bool {
        self.nexthops.contains(nexthop)
    }

    /// Returns the number of next-hops in the group.
    pub fn len(&self) -> usize {
        self.nexthops.len()
    }

    /// Returns true if the group is empty.
    pub fn is_empty(&self) -> bool {
        self.nexthops.is_empty()
    }

    /// Returns an iterator over the next-hops.
    pub fn iter(&self) -> impl Iterator<Item = &NextHopKey> {
        self.nexthops.iter()
    }

    /// Returns the next-hops as a reference.
    pub fn nexthops(&self) -> &BTreeSet<NextHopKey> {
        &self.nexthops
    }

    /// Returns true if this is an ECMP group (more than one next-hop).
    pub fn is_ecmp(&self) -> bool {
        self.nexthops.len() > 1
    }

    /// Returns true if any next-hop is an overlay (VxLAN) next-hop.
    pub fn has_overlay(&self) -> bool {
        self.nexthops.iter().any(|nh| nh.is_overlay())
    }

    /// Returns true if any next-hop is an MPLS next-hop.
    pub fn has_mpls(&self) -> bool {
        self.nexthops.iter().any(|nh| nh.is_mpls())
    }
}

impl Default for NextHopGroupKey {
    fn default() -> Self {
        Self::new()
    }
}

impl Hash for NextHopGroupKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash each nexthop in sorted order
        for nh in &self.nexthops {
            nh.hash(state);
        }
    }
}

impl fmt::Display for NextHopGroupKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let nexthops: Vec<_> = self.nexthops.iter().map(|nh| nh.to_string()).collect();
        write!(f, "{}", nexthops.join(","))
    }
}

/// Error when parsing a NextHopGroupKey.
#[derive(Debug, Clone)]
pub struct ParseNextHopGroupKeyError {
    pub message: String,
}

impl fmt::Display for ParseNextHopGroupKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid next-hop group key: {}", self.message)
    }
}

impl std::error::Error for ParseNextHopGroupKeyError {}

impl FromStr for NextHopGroupKey {
    type Err = ParseNextHopGroupKeyError;

    /// Parses a next-hop group key from a comma-separated string.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.trim().is_empty() {
            return Ok(Self::new());
        }

        let mut nexthops = BTreeSet::new();
        for part in s.split(',') {
            let nh = part.trim().parse().map_err(|e| ParseNextHopGroupKeyError {
                message: format!("{}", e),
            })?;
            nexthops.insert(nh);
        }
        Ok(Self { nexthops })
    }
}

// Implement Ord for NextHopKey to allow BTreeSet
impl Ord for NextHopKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_string().cmp(&other.to_string())
    }
}

impl PartialOrd for NextHopKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Entry for a next-hop group member.
#[derive(Debug, Clone)]
pub struct NextHopGroupMemberEntry {
    /// SAI object ID of the individual next-hop.
    pub next_hop_id: RawSaiObjectId,
    /// Sequence ID for ordered ECMP.
    pub seq_id: u32,
    /// Weight for weighted ECMP.
    pub weight: u32,
}

impl NextHopGroupMemberEntry {
    /// Creates a new member entry.
    pub fn new(next_hop_id: RawSaiObjectId) -> Self {
        Self {
            next_hop_id,
            seq_id: 0,
            weight: 1,
        }
    }

    /// Creates a member entry with sequence ID.
    pub fn with_seq_id(mut self, seq_id: u32) -> Self {
        self.seq_id = seq_id;
        self
    }

    /// Creates a member entry with weight.
    pub fn with_weight(mut self, weight: u32) -> Self {
        self.weight = weight;
        self
    }
}

/// Entry for a next-hop group in the synced table.
///
/// This is the Rust equivalent of C++ `NextHopGroupEntry` with atomic
/// reference counting to prevent data races.
#[derive(Debug)]
pub struct NextHopGroupEntry {
    /// SAI object ID for the next-hop group.
    next_hop_group_id: RawSaiObjectId,
    /// Reference count - uses atomic for thread safety.
    /// This tracks how many routes point to this NHG.
    ref_count: AtomicU32,
    /// Active members: NextHopKey → NextHopGroupMemberEntry.
    nhopgroup_members: HashMap<NextHopKey, NextHopGroupMemberEntry>,
    /// Members used when default route NH swap is active.
    default_route_nhopgroup_members: HashMap<NextHopKey, NextHopGroupMemberEntry>,
    /// Count of installed members.
    nh_member_install_count: u32,
    /// Whether this NHG is eligible for default route NH swap.
    eligible_for_default_route_nh_swap: bool,
    /// Whether this NHG is currently swapped with default route.
    is_default_route_nh_swap: bool,
}

impl NextHopGroupEntry {
    /// Creates a new next-hop group entry with the given SAI ID.
    ///
    /// Reference count is initialized to 0 - routes will increment it.
    pub fn new(next_hop_group_id: RawSaiObjectId) -> Self {
        Self {
            next_hop_group_id,
            ref_count: AtomicU32::new(0),
            nhopgroup_members: HashMap::new(),
            default_route_nhopgroup_members: HashMap::new(),
            nh_member_install_count: 0,
            eligible_for_default_route_nh_swap: false,
            is_default_route_nh_swap: false,
        }
    }

    /// Returns the SAI object ID.
    pub fn sai_id(&self) -> RawSaiObjectId {
        self.next_hop_group_id
    }

    /// Returns the current reference count.
    pub fn ref_count(&self) -> u32 {
        self.ref_count.load(Ordering::SeqCst)
    }

    /// Increments the reference count and returns the new value.
    ///
    /// This is the safe replacement for `m_syncdNextHopGroups[key].ref_count++`.
    /// Unlike C++, this can only be called on an existing entry.
    pub fn increment_ref(&self) -> u32 {
        self.ref_count.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Decrements the reference count and returns the new value.
    ///
    /// Returns 0 if the count was already 0 (underflow protection).
    pub fn decrement_ref(&self) -> u32 {
        // Use fetch_update to prevent underflow
        let result = self
            .ref_count
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                if current > 0 {
                    Some(current - 1)
                } else {
                    None // Don't update if already 0
                }
            });

        match result {
            Ok(prev) => prev - 1,    // Successfully decremented
            Err(current) => current, // Was already 0
        }
    }

    /// Returns true if the reference count is zero.
    pub fn is_ref_count_zero(&self) -> bool {
        self.ref_count.load(Ordering::SeqCst) == 0
    }

    /// Returns the active members.
    pub fn members(&self) -> &HashMap<NextHopKey, NextHopGroupMemberEntry> {
        &self.nhopgroup_members
    }

    /// Returns a mutable reference to the active members.
    pub fn members_mut(&mut self) -> &mut HashMap<NextHopKey, NextHopGroupMemberEntry> {
        &mut self.nhopgroup_members
    }

    /// Adds a member to the group.
    pub fn add_member(&mut self, key: NextHopKey, entry: NextHopGroupMemberEntry) {
        self.nhopgroup_members.insert(key, entry);
    }

    /// Removes a member from the group.
    pub fn remove_member(&mut self, key: &NextHopKey) -> Option<NextHopGroupMemberEntry> {
        self.nhopgroup_members.remove(key)
    }

    /// Returns the count of installed members.
    pub fn installed_member_count(&self) -> u32 {
        self.nh_member_install_count
    }

    /// Sets the count of installed members.
    pub fn set_installed_member_count(&mut self, count: u32) {
        self.nh_member_install_count = count;
    }

    /// Returns whether this NHG is eligible for default route NH swap.
    pub fn is_eligible_for_default_route_swap(&self) -> bool {
        self.eligible_for_default_route_nh_swap
    }

    /// Sets whether this NHG is eligible for default route NH swap.
    pub fn set_eligible_for_default_route_swap(&mut self, eligible: bool) {
        self.eligible_for_default_route_nh_swap = eligible;
    }

    /// Returns whether this NHG is currently swapped with default route.
    pub fn is_default_route_swap_active(&self) -> bool {
        self.is_default_route_nh_swap
    }

    /// Sets whether this NHG is currently swapped with default route.
    pub fn set_default_route_swap_active(&mut self, active: bool) {
        self.is_default_route_nh_swap = active;
    }

    /// Returns the default route swap members.
    pub fn default_route_members(&self) -> &HashMap<NextHopKey, NextHopGroupMemberEntry> {
        &self.default_route_nhopgroup_members
    }

    /// Returns a mutable reference to the default route swap members.
    pub fn default_route_members_mut(
        &mut self,
    ) -> &mut HashMap<NextHopKey, NextHopGroupMemberEntry> {
        &mut self.default_route_nhopgroup_members
    }
}

/// Table of next-hop groups indexed by their key.
///
/// This uses `sonic_orch_common::SyncMap` internally to prevent
/// auto-vivification bugs.
pub type NextHopGroupTable = sonic_orch_common::SyncMap<NextHopGroupKey, NextHopGroupEntry>;

#[cfg(test)]
mod tests {
    use super::*;
    use sonic_types::IpAddress;
    use std::net::Ipv4Addr;

    fn make_nexthop(ip: &str, alias: &str) -> NextHopKey {
        NextHopKey::new(IpAddress::V4(ip.parse::<Ipv4Addr>().unwrap().into()), alias)
    }

    #[test]
    fn test_nhg_key_new() {
        let key = NextHopGroupKey::new();
        assert!(key.is_empty());
        assert!(!key.is_ecmp());
    }

    #[test]
    fn test_nhg_key_single() {
        let nh = make_nexthop("192.168.1.1", "Ethernet0");
        let key = NextHopGroupKey::single(nh.clone());
        assert_eq!(key.len(), 1);
        assert!(!key.is_ecmp());
        assert!(key.contains(&nh));
    }

    #[test]
    fn test_nhg_key_ecmp() {
        let nh1 = make_nexthop("192.168.1.1", "Ethernet0");
        let nh2 = make_nexthop("192.168.1.2", "Ethernet4");
        let key = NextHopGroupKey::from_nexthops([nh1.clone(), nh2.clone()]);
        assert_eq!(key.len(), 2);
        assert!(key.is_ecmp());
        assert!(key.contains(&nh1));
        assert!(key.contains(&nh2));
    }

    #[test]
    fn test_nhg_key_display() {
        let nh1 = make_nexthop("192.168.1.1", "Ethernet0");
        let nh2 = make_nexthop("192.168.1.2", "Ethernet4");
        let key = NextHopGroupKey::from_nexthops([nh1, nh2]);
        let display = key.to_string();
        // Order is deterministic due to BTreeSet
        assert!(display.contains("192.168.1.1@Ethernet0"));
        assert!(display.contains("192.168.1.2@Ethernet4"));
    }

    #[test]
    fn test_nhg_key_parse() {
        let key: NextHopGroupKey = "192.168.1.1@Ethernet0,192.168.1.2@Ethernet4"
            .parse()
            .unwrap();
        assert_eq!(key.len(), 2);
        assert!(key.is_ecmp());

        let empty: NextHopGroupKey = "".parse().unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_nhg_entry_ref_count() {
        let entry = NextHopGroupEntry::new(0x1234);

        assert_eq!(entry.ref_count(), 0);
        assert!(entry.is_ref_count_zero());

        // Increment
        assert_eq!(entry.increment_ref(), 1);
        assert_eq!(entry.ref_count(), 1);
        assert!(!entry.is_ref_count_zero());

        // Increment again
        assert_eq!(entry.increment_ref(), 2);
        assert_eq!(entry.ref_count(), 2);

        // Decrement
        assert_eq!(entry.decrement_ref(), 1);
        assert_eq!(entry.ref_count(), 1);

        // Decrement to zero
        assert_eq!(entry.decrement_ref(), 0);
        assert!(entry.is_ref_count_zero());
    }

    #[test]
    fn test_nhg_entry_underflow_protection() {
        let entry = NextHopGroupEntry::new(0x1234);

        assert_eq!(entry.ref_count(), 0);

        // Try to decrement below zero - should stay at 0
        assert_eq!(entry.decrement_ref(), 0);
        assert_eq!(entry.decrement_ref(), 0);
        assert_eq!(entry.ref_count(), 0);
    }

    #[test]
    fn test_nhg_entry_members() {
        let mut entry = NextHopGroupEntry::new(0x1234);

        let nh = make_nexthop("192.168.1.1", "Ethernet0");
        entry.add_member(nh.clone(), NextHopGroupMemberEntry::new(0x5678));

        assert!(entry.members().contains_key(&nh));
        assert_eq!(entry.members().get(&nh).unwrap().next_hop_id, 0x5678);

        entry.remove_member(&nh);
        assert!(!entry.members().contains_key(&nh));
    }

    #[test]
    fn test_nhg_table_no_auto_vivification() {
        use sonic_orch_common::SyncMap;

        let table: SyncMap<NextHopGroupKey, NextHopGroupEntry> = SyncMap::new();
        let key = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));

        // Getting a non-existent key should return None, not create it
        assert!(table.get(&key).is_none());

        // Table should still be empty
        assert!(table.is_empty());
    }
}
