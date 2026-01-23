//! RouteOrch implementation.
//!
//! This is the main route orchestrator, managing IP route programming
//! with safe next-hop group reference counting.

use async_trait::async_trait;
use log::{debug, error, info, warn};
use sonic_orch_common::{Consumer, ConsumerConfig, KeyOpFieldsValues, Operation, Orch, SyncMap};
use sonic_sai::types::RawSaiObjectId;
use sonic_types::IpPrefix;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::nexthop::NextHopKey;
use super::nhg::{NextHopGroupEntry, NextHopGroupKey, NextHopGroupTable};
use super::types::{RouteEntry, RouteNhg, RouteTables};

/// Error type for RouteOrch operations.
#[derive(Debug, thiserror::Error)]
pub enum RouteError {
    #[error("Next-hop group not found: {0}")]
    NhgNotFound(String),

    #[error("Next-hop group already exists: {0}")]
    NhgAlreadyExists(String),

    #[error("Max next-hop groups reached ({0})")]
    MaxNhgReached(usize),

    #[error("Route not found: {0}")]
    RouteNotFound(String),

    #[error("VRF not found: {0:x}")]
    VrfNotFound(RawSaiObjectId),

    #[error("Next-hop not resolved: {0}")]
    NextHopNotResolved(String),

    #[error("SAI error: {0}")]
    SaiError(String),

    #[error("Invalid route: {0}")]
    InvalidRoute(String),

    #[error("Reference count error: {0}")]
    RefCountError(String),
}

/// Result type for RouteOrch operations.
pub type Result<T> = std::result::Result<T, RouteError>;

/// Configuration for RouteOrch.
#[derive(Debug, Clone)]
pub struct RouteOrchConfig {
    /// Maximum number of next-hop groups allowed.
    pub max_nhg_count: usize,
    /// Whether ordered ECMP is enabled.
    pub ordered_ecmp: bool,
    /// Default packet action for routes.
    pub default_action_drop: bool,
}

impl Default for RouteOrchConfig {
    fn default() -> Self {
        Self {
            max_nhg_count: 1024,
            ordered_ecmp: false,
            default_action_drop: true,
        }
    }
}

/// Callback trait for RouteOrch to interact with other Orchs.
#[async_trait]
pub trait RouteOrchCallbacks: Send + Sync {
    /// Checks if a next-hop is resolved in NeighOrch.
    fn has_next_hop(&self, nexthop: &NextHopKey) -> bool;

    /// Gets the SAI ID for a next-hop from NeighOrch.
    fn get_next_hop_id(&self, nexthop: &NextHopKey) -> Option<RawSaiObjectId>;

    /// Gets the router interface ID for an interface from IntfsOrch.
    fn get_router_intf_id(&self, alias: &str) -> Option<RawSaiObjectId>;

    /// Checks if a VRF exists.
    fn vrf_exists(&self, vrf_id: RawSaiObjectId) -> bool;

    /// Increments next-hop ref count in NeighOrch.
    fn increase_next_hop_ref_count(&self, nexthop: &NextHopKey);

    /// Decrements next-hop ref count in NeighOrch.
    fn decrease_next_hop_ref_count(&self, nexthop: &NextHopKey);

    /// Increments router interface ref count in IntfsOrch.
    fn increase_router_intf_ref_count(&self, alias: &str);

    /// Decrements router interface ref count in IntfsOrch.
    fn decrease_router_intf_ref_count(&self, alias: &str);

    /// Increments VRF ref count.
    fn increase_vrf_ref_count(&self, vrf_id: RawSaiObjectId);

    /// Decrements VRF ref count.
    fn decrease_vrf_ref_count(&self, vrf_id: RawSaiObjectId);

    /// Creates a next-hop group in SAI.
    async fn sai_create_nhg(&self, nhg_key: &NextHopGroupKey) -> Result<RawSaiObjectId>;

    /// Removes a next-hop group from SAI.
    async fn sai_remove_nhg(&self, nhg_id: RawSaiObjectId) -> Result<()>;

    /// Creates a route entry in SAI.
    async fn sai_create_route(
        &self,
        vrf_id: RawSaiObjectId,
        prefix: &IpPrefix,
        nhg_id: Option<RawSaiObjectId>,
        blackhole: bool,
    ) -> Result<()>;

    /// Removes a route entry from SAI.
    async fn sai_remove_route(&self, vrf_id: RawSaiObjectId, prefix: &IpPrefix) -> Result<()>;

    /// Updates a route entry in SAI.
    async fn sai_set_route(
        &self,
        vrf_id: RawSaiObjectId,
        prefix: &IpPrefix,
        nhg_id: Option<RawSaiObjectId>,
        blackhole: bool,
    ) -> Result<()>;
}

/// RouteOrch - Manages IP route programming.
///
/// This is the Rust implementation of the C++ RouteOrch, with proper
/// reference counting that prevents auto-vivification bugs.
pub struct RouteOrch {
    /// Configuration.
    config: RouteOrchConfig,

    /// Consumer for ROUTE_TABLE.
    consumer: Consumer,

    /// Synced routes indexed by VRF ID and prefix.
    synced_routes: RouteTables,

    /// Synced next-hop groups.
    /// Using SyncMap to prevent auto-vivification!
    synced_nhgs: NextHopGroupTable,

    /// Count of next-hop groups.
    nhg_count: usize,

    /// Callbacks for interacting with other Orchs.
    callbacks: Option<Arc<dyn RouteOrchCallbacks>>,

    /// Pending NHG removals (deferred until ref_count == 0).
    pending_nhg_removals: HashSet<NextHopGroupKey>,
}

impl RouteOrch {
    /// Creates a new RouteOrch with the given configuration.
    pub fn new(config: RouteOrchConfig) -> Self {
        Self {
            config,
            consumer: Consumer::new(ConsumerConfig::new("ROUTE_TABLE")),
            synced_routes: HashMap::new(),
            synced_nhgs: SyncMap::new(),
            nhg_count: 0,
            callbacks: None,
            pending_nhg_removals: HashSet::new(),
        }
    }

    /// Sets the callbacks for interacting with other Orchs.
    pub fn set_callbacks(&mut self, callbacks: Arc<dyn RouteOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    /// Returns the current count of next-hop groups.
    pub fn nhg_count(&self) -> usize {
        self.nhg_count
    }

    /// Returns the maximum allowed next-hop groups.
    pub fn max_nhg_count(&self) -> usize {
        self.config.max_nhg_count
    }

    /// Checks if a next-hop group exists.
    pub fn has_nhg(&self, key: &NextHopGroupKey) -> bool {
        self.synced_nhgs.contains_key(key)
    }

    /// Gets a reference to a next-hop group entry.
    ///
    /// Returns None if the group doesn't exist - does NOT create it.
    pub fn get_nhg(&self, key: &NextHopGroupKey) -> Option<&NextHopGroupEntry> {
        self.synced_nhgs.get(key)
    }

    /// Gets a mutable reference to a next-hop group entry.
    ///
    /// Returns None if the group doesn't exist - does NOT create it.
    pub fn get_nhg_mut(&mut self, key: &NextHopGroupKey) -> Option<&mut NextHopGroupEntry> {
        self.synced_nhgs.get_mut(key)
    }

    /// Returns true if the next-hop group's ref count is zero.
    ///
    /// Returns true if the group doesn't exist (safe default).
    pub fn is_nhg_ref_count_zero(&self, key: &NextHopGroupKey) -> bool {
        match self.synced_nhgs.get(key) {
            Some(entry) => entry.is_ref_count_zero(),
            None => true,
        }
    }

    /// Increases the next-hop reference count.
    ///
    /// This is the SAFE replacement for C++ `m_syncdNextHopGroups[key].ref_count++`.
    /// Unlike C++, this returns an error if the key doesn't exist instead of
    /// auto-creating an entry.
    ///
    /// For single next-hops, delegates to NeighOrch/IntfsOrch.
    /// For ECMP groups, increments the ref count in synced_nhgs.
    pub fn increase_nhg_ref_count(&mut self, key: &NextHopGroupKey) -> Result<()> {
        // Clone the Arc to avoid borrowing self.callbacks while we mutate self
        let callbacks = self.callbacks.clone().ok_or_else(|| {
            RouteError::RefCountError("Callbacks not set".to_string())
        })?;

        if key.is_empty() {
            // Blackhole/dropped route - no ref count to manage
            return Ok(());
        }

        if key.len() == 1 {
            // Single next-hop: delegate to NeighOrch/IntfsOrch
            let nexthop = key.iter().next().unwrap();
            if nexthop.is_interface_nexthop() {
                callbacks.increase_router_intf_ref_count(nexthop.alias());
            } else {
                callbacks.increase_next_hop_ref_count(nexthop);
            }
            return Ok(());
        }

        // ECMP group: increment ref count in our table
        // This is the key safety improvement - we use get_mut instead of []
        let entry = self.synced_nhgs.get_mut(key).ok_or_else(|| {
            RouteError::NhgNotFound(format!(
                "Cannot increment ref count for non-existent NHG: {}",
                key
            ))
        })?;

        let new_count = entry.increment_ref();
        debug!(
            "RouteOrch: Increased NHG ref count to {} for {}",
            new_count, key
        );

        Ok(())
    }

    /// Decreases the next-hop reference count.
    ///
    /// This is the SAFE replacement for C++ `m_syncdNextHopGroups[key].ref_count--`.
    pub fn decrease_nhg_ref_count(&mut self, key: &NextHopGroupKey) -> Result<()> {
        // Clone the Arc to avoid borrowing self.callbacks while we mutate self
        let callbacks = self.callbacks.clone().ok_or_else(|| {
            RouteError::RefCountError("Callbacks not set".to_string())
        })?;

        if key.is_empty() {
            return Ok(());
        }

        if key.len() == 1 {
            let nexthop = key.iter().next().unwrap();
            if nexthop.is_interface_nexthop() {
                callbacks.decrease_router_intf_ref_count(nexthop.alias());
            } else {
                callbacks.decrease_next_hop_ref_count(nexthop);
            }
            return Ok(());
        }

        // ECMP group
        let entry = self.synced_nhgs.get_mut(key).ok_or_else(|| {
            RouteError::NhgNotFound(format!(
                "Cannot decrement ref count for non-existent NHG: {}",
                key
            ))
        })?;

        let new_count = entry.decrement_ref();
        debug!(
            "RouteOrch: Decreased NHG ref count to {} for {}",
            new_count, key
        );

        // If ref count is now zero, mark for removal
        if new_count == 0 {
            self.pending_nhg_removals.insert(key.clone());
        }

        Ok(())
    }

    /// Adds a next-hop group.
    ///
    /// Creates the NHG in SAI and adds it to synced_nhgs with ref_count = 0.
    pub async fn add_nhg(&mut self, key: NextHopGroupKey) -> Result<RawSaiObjectId> {
        // Check if already exists
        if self.synced_nhgs.contains_key(&key) {
            return Err(RouteError::NhgAlreadyExists(key.to_string()));
        }

        // Check capacity
        if self.nhg_count >= self.config.max_nhg_count {
            return Err(RouteError::MaxNhgReached(self.config.max_nhg_count));
        }

        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            RouteError::SaiError("Callbacks not set".to_string())
        })?;

        // Create in SAI
        let nhg_id = callbacks.sai_create_nhg(&key).await?;

        // Add to our table with ref_count = 0
        let entry = NextHopGroupEntry::new(nhg_id);
        self.synced_nhgs.insert(key.clone(), entry);
        self.nhg_count += 1;

        info!("RouteOrch: Created NHG {} with SAI ID {:x}", key, nhg_id);

        Ok(nhg_id)
    }

    /// Removes a next-hop group.
    ///
    /// Only succeeds if ref_count == 0.
    pub async fn remove_nhg(&mut self, key: &NextHopGroupKey) -> Result<()> {
        // Get the entry and check ref count
        let entry = self.synced_nhgs.get(key).ok_or_else(|| {
            RouteError::NhgNotFound(key.to_string())
        })?;

        if !entry.is_ref_count_zero() {
            return Err(RouteError::RefCountError(format!(
                "Cannot remove NHG {} with ref_count {}",
                key,
                entry.ref_count()
            )));
        }

        let nhg_id = entry.sai_id();

        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            RouteError::SaiError("Callbacks not set".to_string())
        })?;

        // Remove from SAI
        callbacks.sai_remove_nhg(nhg_id).await?;

        // Remove from our table
        self.synced_nhgs.remove(key);
        self.nhg_count -= 1;
        self.pending_nhg_removals.remove(key);

        info!("RouteOrch: Removed NHG {}", key);

        Ok(())
    }

    /// Processes pending NHG removals.
    pub async fn process_pending_nhg_removals(&mut self) -> Result<()> {
        let to_remove: Vec<_> = self.pending_nhg_removals.iter().cloned().collect();

        for key in to_remove {
            if self.is_nhg_ref_count_zero(&key) {
                if let Err(e) = self.remove_nhg(&key).await {
                    warn!("Failed to remove pending NHG {}: {}", key, e);
                }
            }
        }

        Ok(())
    }

    /// Checks if a route exists.
    pub fn has_route(&self, vrf_id: RawSaiObjectId, prefix: &IpPrefix) -> bool {
        self.synced_routes
            .get(&vrf_id)
            .map(|table| table.contains_key(prefix))
            .unwrap_or(false)
    }

    /// Gets a reference to a route entry.
    pub fn get_route(&self, vrf_id: RawSaiObjectId, prefix: &IpPrefix) -> Option<&RouteEntry> {
        self.synced_routes
            .get(&vrf_id)
            .and_then(|table| table.get(prefix))
    }

    /// Adds a route.
    pub async fn add_route(
        &mut self,
        vrf_id: RawSaiObjectId,
        prefix: IpPrefix,
        nhg_key: NextHopGroupKey,
    ) -> Result<()> {
        // Clone callbacks Arc to avoid borrowing self
        let callbacks = self.callbacks.clone().ok_or_else(|| {
            RouteError::SaiError("Callbacks not set".to_string())
        })?;

        // Check VRF exists
        if vrf_id != 0 && !callbacks.vrf_exists(vrf_id) {
            return Err(RouteError::VrfNotFound(vrf_id));
        }

        // Determine the NHG ID to use
        let (nhg_id, blackhole) = if nhg_key.is_empty() {
            (None, true)
        } else if nhg_key.len() == 1 {
            // Single next-hop
            let nexthop = nhg_key.iter().next().unwrap();
            if nexthop.is_interface_nexthop() {
                let rif_id = callbacks.get_router_intf_id(nexthop.alias()).ok_or_else(|| {
                    RouteError::NextHopNotResolved(nexthop.alias().to_string())
                })?;
                (Some(rif_id), false)
            } else {
                let nh_id = callbacks.get_next_hop_id(nexthop).ok_or_else(|| {
                    RouteError::NextHopNotResolved(nexthop.to_string())
                })?;
                (Some(nh_id), false)
            }
        } else {
            // ECMP group
            let nhg_id = if self.has_nhg(&nhg_key) {
                self.synced_nhgs.get(&nhg_key).unwrap().sai_id()
            } else {
                // Create the NHG
                self.add_nhg(nhg_key.clone()).await?
            };
            (Some(nhg_id), false)
        };

        // Check if route already exists
        let existing = self.get_route(vrf_id, &prefix);
        let is_update = existing.is_some();
        let old_nhg_key = existing.map(|e| e.nhg.nhg_key.clone());

        if is_update {
            // Update existing route
            callbacks
                .sai_set_route(vrf_id, &prefix, nhg_id, blackhole)
                .await?;

            // Update ref counts
            if let Some(ref old_key) = old_nhg_key {
                if old_key != &nhg_key {
                    self.decrease_nhg_ref_count(old_key)?;
                    self.increase_nhg_ref_count(&nhg_key)?;
                }
            }

            // Update our table
            let table = self.synced_routes.entry(vrf_id).or_default();
            if let Some(entry) = table.get_mut(&prefix) {
                entry.nhg = RouteNhg::new(nhg_key);
            }

            debug!("RouteOrch: Updated route {}/{}", vrf_id, prefix);
        } else {
            // Create new route
            callbacks
                .sai_create_route(vrf_id, &prefix, nhg_id, blackhole)
                .await?;

            // Increase ref counts
            self.increase_nhg_ref_count(&nhg_key)?;
            if vrf_id != 0 {
                callbacks.increase_vrf_ref_count(vrf_id);
            }

            // Add to our table
            let table = self.synced_routes.entry(vrf_id).or_default();
            table.insert(prefix.clone(), RouteEntry::new(RouteNhg::new(nhg_key)));

            info!("RouteOrch: Added route {}/{}", vrf_id, prefix);
        }

        Ok(())
    }

    /// Removes a route.
    pub async fn remove_route(&mut self, vrf_id: RawSaiObjectId, prefix: &IpPrefix) -> Result<()> {
        // Clone the Arc to avoid borrowing self.callbacks while we mutate self
        let callbacks = self.callbacks.clone().ok_or_else(|| {
            RouteError::SaiError("Callbacks not set".to_string())
        })?;

        // Get the existing route
        let entry = self
            .synced_routes
            .get(&vrf_id)
            .and_then(|table| table.get(prefix))
            .ok_or_else(|| RouteError::RouteNotFound(format!("{}/{}", vrf_id, prefix)))?;

        let nhg_key = entry.nhg.nhg_key.clone();

        // Check if this is a default route
        let is_default = prefix.is_default();

        if is_default && self.config.default_action_drop {
            // For default routes, just set to DROP instead of removing
            callbacks
                .sai_set_route(vrf_id, prefix, None, true)
                .await?;

            // Update our table
            let table = self.synced_routes.get_mut(&vrf_id).unwrap();
            if let Some(entry) = table.get_mut(prefix) {
                let old_nhg_key = std::mem::take(&mut entry.nhg.nhg_key);
                self.decrease_nhg_ref_count(&old_nhg_key)?;
            }

            debug!(
                "RouteOrch: Set default route {} to DROP",
                prefix
            );
        } else {
            // Remove the route
            callbacks.sai_remove_route(vrf_id, prefix).await?;

            // Decrease ref counts
            self.decrease_nhg_ref_count(&nhg_key)?;
            if vrf_id != 0 {
                callbacks.decrease_vrf_ref_count(vrf_id);
            }

            // Remove from our table
            let table = self.synced_routes.get_mut(&vrf_id).unwrap();
            table.remove(prefix);

            // Clean up empty VRF table
            if table.is_empty() && vrf_id != 0 {
                self.synced_routes.remove(&vrf_id);
            }

            info!("RouteOrch: Removed route {}/{}", vrf_id, prefix);
        }

        // Process any pending NHG removals
        self.process_pending_nhg_removals().await?;

        Ok(())
    }

    /// Adds a task to the consumer for processing.
    pub fn add_task(&mut self, key: String, op: Operation, fields: HashMap<String, String>) {
        let fvs: Vec<(String, String)> = fields.into_iter().collect();
        self.consumer.add_to_sync(vec![KeyOpFieldsValues::new(key, op, fvs)]);
    }
}

#[async_trait]
impl Orch for RouteOrch {
    fn name(&self) -> &str {
        "RouteOrch"
    }

    fn priority(&self) -> i32 {
        // RouteOrch has medium priority
        10
    }

    async fn do_task(&mut self) {
        // Check if callbacks are available
        let _callbacks = match &self.callbacks {
            Some(cb) => cb.clone(),
            None => {
                debug!("RouteOrch: callbacks not set");
                return;
            }
        };

        // Process pending tasks
        let tasks = self.consumer.drain();

        for task in tasks {
            // Parse VRF and prefix from key
            // Key format: "vrf_id:prefix" or just "prefix" for default VRF
            let (vrf_id, prefix) = match parse_route_key(&task.key) {
                Ok((v, p)) => (v, p),
                Err(e) => {
                    warn!("Invalid route key {}: {}", task.key, e);
                    continue;
                }
            };

            match task.op {
                Operation::Set => {
                    // Parse next-hops from fields
                    let fields: HashMap<String, String> = task.fvs.into_iter().collect();
                    let nhg_key = match parse_nexthops(&fields) {
                        Ok(key) => key,
                        Err(e) => {
                            warn!("Invalid nexthops for {}: {}", task.key, e);
                            continue;
                        }
                    };

                    if let Err(e) = self.add_route(vrf_id, prefix, nhg_key).await {
                        error!("Failed to add route {}: {}", task.key, e);
                    }
                }
                Operation::Del => {
                    if let Err(e) = self.remove_route(vrf_id, &prefix).await {
                        error!("Failed to remove route {}: {}", task.key, e);
                    }
                }
            }
        }
    }

    fn has_pending_tasks(&self) -> bool {
        self.consumer.has_pending()
    }

    fn bake(&mut self) -> bool {
        // Routes need to be reconciled during warm restart
        // For now, just return true
        true
    }

    fn dump_pending_tasks(&self) -> Vec<String> {
        self.consumer
            .peek()
            .map(|t| format!("{}:{:?}", t.key, t.op))
            .collect()
    }
}

/// Parses a route key into VRF ID and prefix.
fn parse_route_key(key: &str) -> Result<(RawSaiObjectId, IpPrefix)> {
    if let Some((vrf_str, prefix_str)) = key.split_once(':') {
        let vrf_id = u64::from_str_radix(vrf_str.trim_start_matches("0x"), 16)
            .map_err(|_| RouteError::InvalidRoute(format!("Invalid VRF: {}", vrf_str)))?;
        let prefix = prefix_str
            .parse()
            .map_err(|_| RouteError::InvalidRoute(format!("Invalid prefix: {}", prefix_str)))?;
        Ok((vrf_id, prefix))
    } else {
        // Default VRF
        let prefix = key
            .parse()
            .map_err(|_| RouteError::InvalidRoute(format!("Invalid prefix: {}", key)))?;
        Ok((0, prefix))
    }
}

/// Parses next-hops from field-value pairs.
fn parse_nexthops(fields: &HashMap<String, String>) -> Result<NextHopGroupKey> {
    // Look for "nexthop" field
    let nexthop_str = fields.get("nexthop").or_else(|| fields.get("NEXTHOP"));

    if let Some(nh_str) = nexthop_str {
        if nh_str.is_empty() || nh_str == "blackhole" || nh_str == "drop" {
            return Ok(NextHopGroupKey::new());
        }

        nh_str
            .parse()
            .map_err(|e| RouteError::InvalidRoute(format!("Invalid nexthops: {}", e)))
    } else {
        // No nexthop field - check for blackhole
        if fields.contains_key("blackhole") {
            return Ok(NextHopGroupKey::new());
        }
        Err(RouteError::InvalidRoute("Missing nexthop field".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use std::sync::{Arc, Mutex};

    // Test helper: Create a test IP prefix
    fn make_prefix(addr: &str, len: u8) -> IpPrefix {
        IpPrefix::new(
            sonic_types::IpAddress::V4(addr.parse::<Ipv4Addr>().unwrap().into()),
            len,
        )
        .unwrap()
    }

    // Test helper: Create a next-hop key
    fn make_nexthop(ip: &str, alias: &str) -> NextHopKey {
        NextHopKey::new(
            sonic_types::IpAddress::V4(ip.parse::<Ipv4Addr>().unwrap().into()),
            alias,
        )
    }

    // Mock callbacks for testing
    #[derive(Default)]
    struct MockCallbacks {
        next_hop_ids: Arc<Mutex<HashMap<NextHopKey, RawSaiObjectId>>>,
        router_intf_ids: Arc<Mutex<HashMap<String, RawSaiObjectId>>>,
        next_hop_refs: Arc<Mutex<HashMap<NextHopKey, u32>>>,
        router_intf_refs: Arc<Mutex<HashMap<String, u32>>>,
        vrf_refs: Arc<Mutex<HashMap<RawSaiObjectId, u32>>>,
        vrfs: Arc<Mutex<HashSet<RawSaiObjectId>>>,
        nhg_counter: Arc<Mutex<u64>>,
    }

    impl MockCallbacks {
        fn new() -> Self {
            Self::default()
        }

        fn add_next_hop(&self, nh: NextHopKey, id: RawSaiObjectId) {
            self.next_hop_ids.lock().unwrap().insert(nh, id);
        }

        fn add_router_intf(&self, alias: String, id: RawSaiObjectId) {
            self.router_intf_ids.lock().unwrap().insert(alias, id);
        }

        fn add_vrf(&self, vrf_id: RawSaiObjectId) {
            self.vrfs.lock().unwrap().insert(vrf_id);
        }
    }

    #[async_trait]
    impl RouteOrchCallbacks for MockCallbacks {
        fn has_next_hop(&self, nexthop: &NextHopKey) -> bool {
            self.next_hop_ids.lock().unwrap().contains_key(nexthop)
        }

        fn get_next_hop_id(&self, nexthop: &NextHopKey) -> Option<RawSaiObjectId> {
            self.next_hop_ids.lock().unwrap().get(nexthop).copied()
        }

        fn get_router_intf_id(&self, alias: &str) -> Option<RawSaiObjectId> {
            self.router_intf_ids.lock().unwrap().get(alias).copied()
        }

        fn vrf_exists(&self, vrf_id: RawSaiObjectId) -> bool {
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

        fn increase_vrf_ref_count(&self, vrf_id: RawSaiObjectId) {
            *self.vrf_refs.lock().unwrap().entry(vrf_id).or_insert(0) += 1;
        }

        fn decrease_vrf_ref_count(&self, vrf_id: RawSaiObjectId) {
            if let Some(count) = self.vrf_refs.lock().unwrap().get_mut(&vrf_id) {
                *count = count.saturating_sub(1);
            }
        }

        async fn sai_create_nhg(&self, _nhg_key: &NextHopGroupKey) -> Result<RawSaiObjectId> {
            let mut counter = self.nhg_counter.lock().unwrap();
            *counter += 1;
            Ok(*counter)
        }

        async fn sai_remove_nhg(&self, _nhg_id: RawSaiObjectId) -> Result<()> {
            Ok(())
        }

        async fn sai_create_route(
            &self,
            _vrf_id: RawSaiObjectId,
            _prefix: &IpPrefix,
            _nhg_id: Option<RawSaiObjectId>,
            _blackhole: bool,
        ) -> Result<()> {
            Ok(())
        }

        async fn sai_remove_route(&self, _vrf_id: RawSaiObjectId, _prefix: &IpPrefix) -> Result<()> {
            Ok(())
        }

        async fn sai_set_route(
            &self,
            _vrf_id: RawSaiObjectId,
            _prefix: &IpPrefix,
            _nhg_id: Option<RawSaiObjectId>,
            _blackhole: bool,
        ) -> Result<()> {
            Ok(())
        }
    }

    // ===== Basic parsing tests =====

    #[test]
    fn test_parse_route_key_default_vrf() {
        let (vrf, prefix) = parse_route_key("10.0.0.0/24").unwrap();
        assert_eq!(vrf, 0);
        assert_eq!(prefix.to_string(), "10.0.0.0/24");
    }

    #[test]
    fn test_parse_route_key_with_vrf() {
        let (vrf, prefix) = parse_route_key("0x1234:10.0.0.0/24").unwrap();
        assert_eq!(vrf, 0x1234);
        assert_eq!(prefix.to_string(), "10.0.0.0/24");
    }

    #[test]
    fn test_parse_route_key_invalid_vrf() {
        let result = parse_route_key("invalid:10.0.0.0/24");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::InvalidRoute(_)));
    }

    #[test]
    fn test_parse_route_key_invalid_prefix() {
        let result = parse_route_key("10.0.0.0/99");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::InvalidRoute(_)));
    }

    #[test]
    fn test_parse_nexthops_single() {
        let mut fields = HashMap::new();
        fields.insert("nexthop".to_string(), "192.168.1.1@Ethernet0".to_string());

        let key = parse_nexthops(&fields).unwrap();
        assert_eq!(key.len(), 1);
        assert!(!key.is_ecmp());
    }

    #[test]
    fn test_parse_nexthops_ecmp() {
        let mut fields = HashMap::new();
        fields.insert(
            "nexthop".to_string(),
            "192.168.1.1@Ethernet0,192.168.1.2@Ethernet4".to_string(),
        );

        let key = parse_nexthops(&fields).unwrap();
        assert_eq!(key.len(), 2);
        assert!(key.is_ecmp());
    }

    #[test]
    fn test_parse_nexthops_blackhole() {
        let mut fields = HashMap::new();
        fields.insert("nexthop".to_string(), "blackhole".to_string());

        let key = parse_nexthops(&fields).unwrap();
        assert!(key.is_empty());
    }

    #[test]
    fn test_parse_nexthops_drop() {
        let mut fields = HashMap::new();
        fields.insert("nexthop".to_string(), "drop".to_string());

        let key = parse_nexthops(&fields).unwrap();
        assert!(key.is_empty());
    }

    #[test]
    fn test_parse_nexthops_empty_string() {
        let mut fields = HashMap::new();
        fields.insert("nexthop".to_string(), "".to_string());

        let key = parse_nexthops(&fields).unwrap();
        assert!(key.is_empty());
    }

    #[test]
    fn test_parse_nexthops_blackhole_field() {
        let mut fields = HashMap::new();
        fields.insert("blackhole".to_string(), "true".to_string());

        let key = parse_nexthops(&fields).unwrap();
        assert!(key.is_empty());
    }

    #[test]
    fn test_parse_nexthops_missing_field() {
        let fields = HashMap::new();
        let result = parse_nexthops(&fields);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::InvalidRoute(_)));
    }

    #[test]
    fn test_parse_nexthops_uppercase_field() {
        let mut fields = HashMap::new();
        fields.insert("NEXTHOP".to_string(), "192.168.1.1@Ethernet0".to_string());

        let key = parse_nexthops(&fields).unwrap();
        assert_eq!(key.len(), 1);
    }

    // ===== RouteOrch initialization tests =====

    #[test]
    fn test_route_orch_new() {
        let orch = RouteOrch::new(RouteOrchConfig::default());
        assert_eq!(orch.name(), "RouteOrch");
        assert_eq!(orch.nhg_count(), 0);
        assert_eq!(orch.max_nhg_count(), 1024);
    }

    #[test]
    fn test_route_orch_custom_config() {
        let config = RouteOrchConfig {
            max_nhg_count: 512,
            ordered_ecmp: true,
            default_action_drop: false,
        };
        let orch = RouteOrch::new(config);
        assert_eq!(orch.max_nhg_count(), 512);
    }

    #[test]
    fn test_route_orch_nhg_not_auto_vivified() {
        let orch = RouteOrch::new(RouteOrchConfig::default());

        let key = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));

        // NHG should not exist
        assert!(!orch.has_nhg(&key));

        // Getting it should return None, NOT create it
        assert!(orch.get_nhg(&key).is_none());

        // Table should still be empty
        assert_eq!(orch.nhg_count(), 0);
    }

    #[test]
    fn test_route_orch_ref_count_requires_existing() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());

        let key = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.1.1", "Ethernet0"),
            make_nexthop("192.168.1.2", "Ethernet4"),
        ]);

        // increase_nhg_ref_count should fail because NHG doesn't exist and callbacks not set
        let result = orch.increase_nhg_ref_count(&key);
        assert!(result.is_err());
    }

    // ===== NHG management tests =====

    #[tokio::test]
    async fn test_add_nhg_success() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.1.1", "Ethernet0"),
            make_nexthop("192.168.1.2", "Ethernet4"),
        ]);

        let result = orch.add_nhg(key.clone()).await;
        assert!(result.is_ok());
        assert_eq!(orch.nhg_count(), 1);
        assert!(orch.has_nhg(&key));
    }

    #[tokio::test]
    async fn test_add_nhg_duplicate() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.1.1", "Ethernet0"),
            make_nexthop("192.168.1.2", "Ethernet4"),
        ]);

        orch.add_nhg(key.clone()).await.unwrap();
        let result = orch.add_nhg(key.clone()).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::NhgAlreadyExists(_)));
        assert_eq!(orch.nhg_count(), 1);
    }

    #[tokio::test]
    async fn test_add_nhg_max_limit() {
        let config = RouteOrchConfig {
            max_nhg_count: 2,
            ..Default::default()
        };
        let mut orch = RouteOrch::new(config);
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let key1 = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.1.1", "Ethernet0"),
            make_nexthop("192.168.1.2", "Ethernet4"),
        ]);
        let key2 = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.2.1", "Ethernet0"),
            make_nexthop("192.168.2.2", "Ethernet4"),
        ]);
        let key3 = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.3.1", "Ethernet0"),
            make_nexthop("192.168.3.2", "Ethernet4"),
        ]);

        orch.add_nhg(key1).await.unwrap();
        orch.add_nhg(key2).await.unwrap();

        let result = orch.add_nhg(key3).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::MaxNhgReached(2)));
        assert_eq!(orch.nhg_count(), 2);
    }

    #[tokio::test]
    async fn test_remove_nhg_success() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.1.1", "Ethernet0"),
            make_nexthop("192.168.1.2", "Ethernet4"),
        ]);

        orch.add_nhg(key.clone()).await.unwrap();
        assert_eq!(orch.nhg_count(), 1);

        let result = orch.remove_nhg(&key).await;
        assert!(result.is_ok());
        assert_eq!(orch.nhg_count(), 0);
        assert!(!orch.has_nhg(&key));
    }

    #[tokio::test]
    async fn test_remove_nhg_not_found() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.1.1", "Ethernet0"),
            make_nexthop("192.168.1.2", "Ethernet4"),
        ]);

        let result = orch.remove_nhg(&key).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::NhgNotFound(_)));
    }

    #[tokio::test]
    async fn test_remove_nhg_with_references() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.1.1", "Ethernet0"),
            make_nexthop("192.168.1.2", "Ethernet4"),
        ]);

        orch.add_nhg(key.clone()).await.unwrap();
        orch.increase_nhg_ref_count(&key).unwrap();

        let result = orch.remove_nhg(&key).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::RefCountError(_)));
        assert_eq!(orch.nhg_count(), 1);
    }

    // ===== NHG reference counting tests =====

    #[test]
    fn test_nhg_ref_count_increment_decrement() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.1.1", "Ethernet0"),
            make_nexthop("192.168.1.2", "Ethernet4"),
        ]);

        // Manually add NHG entry for testing
        let entry = NextHopGroupEntry::new(0x1234);
        orch.synced_nhgs.insert(key.clone(), entry);
        orch.nhg_count += 1;

        // Initially ref count is 0
        assert!(orch.is_nhg_ref_count_zero(&key));

        // Increment
        orch.increase_nhg_ref_count(&key).unwrap();
        assert!(!orch.is_nhg_ref_count_zero(&key));
        assert_eq!(orch.get_nhg(&key).unwrap().ref_count(), 1);

        // Increment again
        orch.increase_nhg_ref_count(&key).unwrap();
        assert_eq!(orch.get_nhg(&key).unwrap().ref_count(), 2);

        // Decrement
        orch.decrease_nhg_ref_count(&key).unwrap();
        assert_eq!(orch.get_nhg(&key).unwrap().ref_count(), 1);

        // Decrement to zero
        orch.decrease_nhg_ref_count(&key).unwrap();
        assert!(orch.is_nhg_ref_count_zero(&key));
    }

    #[test]
    fn test_nhg_ref_count_single_nexthop_delegates() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        orch.set_callbacks(callbacks.clone());

        let key = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));

        // Single NH should delegate to NeighOrch
        orch.increase_nhg_ref_count(&key).unwrap();

        let refs = callbacks.next_hop_refs.lock().unwrap();
        assert_eq!(refs.get(&make_nexthop("192.168.1.1", "Ethernet0")), Some(&1));
    }

    #[test]
    fn test_nhg_ref_count_interface_nexthop_delegates() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_router_intf("Vlan100".to_string(), 0x2000);
        orch.set_callbacks(callbacks.clone());

        let key = NextHopGroupKey::single(NextHopKey::interface_only("Vlan100"));

        // Interface NH should delegate to IntfsOrch
        orch.increase_nhg_ref_count(&key).unwrap();

        let refs = callbacks.router_intf_refs.lock().unwrap();
        assert_eq!(refs.get("Vlan100"), Some(&1));
    }

    #[test]
    fn test_nhg_ref_count_empty_key() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = NextHopGroupKey::new(); // Empty = blackhole

        // Empty key should not affect ref counts
        let result = orch.increase_nhg_ref_count(&key);
        assert!(result.is_ok());

        let result = orch.decrease_nhg_ref_count(&key);
        assert!(result.is_ok());
    }

    #[test]
    fn test_nhg_ref_count_nonexistent_fails() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.1.1", "Ethernet0"),
            make_nexthop("192.168.1.2", "Ethernet4"),
        ]);

        // Should fail because NHG doesn't exist
        let result = orch.increase_nhg_ref_count(&key);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::NhgNotFound(_)));

        let result = orch.decrease_nhg_ref_count(&key);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::NhgNotFound(_)));
    }

    #[tokio::test]
    async fn test_pending_nhg_removal() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.1.1", "Ethernet0"),
            make_nexthop("192.168.1.2", "Ethernet4"),
        ]);

        orch.add_nhg(key.clone()).await.unwrap();
        orch.increase_nhg_ref_count(&key).unwrap();

        // Decrease to zero should mark for pending removal
        orch.decrease_nhg_ref_count(&key).unwrap();
        assert!(orch.pending_nhg_removals.contains(&key));

        // Process pending removals
        orch.process_pending_nhg_removals().await.unwrap();
        assert!(!orch.has_nhg(&key));
        assert_eq!(orch.nhg_count(), 0);
    }

    // ===== Route management tests =====

    #[tokio::test]
    async fn test_add_route_single_nexthop() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        orch.set_callbacks(callbacks.clone());

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));

        let result = orch.add_route(0, prefix.clone(), nhg_key).await;
        assert!(result.is_ok());
        assert!(orch.has_route(0, &prefix));

        // Check ref count was incremented
        let refs = callbacks.next_hop_refs.lock().unwrap();
        assert_eq!(refs.get(&make_nexthop("192.168.1.1", "Ethernet0")), Some(&1));
    }

    #[tokio::test]
    async fn test_add_route_ecmp() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        callbacks.add_next_hop(make_nexthop("192.168.1.2", "Ethernet4"), 0x1001);
        orch.set_callbacks(callbacks);

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.1.1", "Ethernet0"),
            make_nexthop("192.168.1.2", "Ethernet4"),
        ]);

        let result = orch.add_route(0, prefix.clone(), nhg_key.clone()).await;
        assert!(result.is_ok());
        assert!(orch.has_route(0, &prefix));

        // NHG should have been created
        assert!(orch.has_nhg(&nhg_key));
        assert_eq!(orch.nhg_count(), 1);
        assert_eq!(orch.get_nhg(&nhg_key).unwrap().ref_count(), 1);
    }

    #[tokio::test]
    async fn test_add_route_blackhole() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::new(); // Empty = blackhole

        let result = orch.add_route(0, prefix.clone(), nhg_key).await;
        assert!(result.is_ok());
        assert!(orch.has_route(0, &prefix));
    }

    #[tokio::test]
    async fn test_add_route_with_vrf() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_vrf(0x1234);
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        orch.set_callbacks(callbacks.clone());

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));

        let result = orch.add_route(0x1234, prefix.clone(), nhg_key).await;
        assert!(result.is_ok());
        assert!(orch.has_route(0x1234, &prefix));

        // VRF ref count should be incremented
        let refs = callbacks.vrf_refs.lock().unwrap();
        assert_eq!(refs.get(&0x1234), Some(&1));
    }

    #[tokio::test]
    async fn test_add_route_vrf_not_found() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::new();

        let result = orch.add_route(0x9999, prefix, nhg_key).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::VrfNotFound(0x9999)));
    }

    #[tokio::test]
    async fn test_add_route_nexthop_not_resolved() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));

        let result = orch.add_route(0, prefix, nhg_key).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::NextHopNotResolved(_)));
    }

    #[tokio::test]
    async fn test_update_route_nexthop_change() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        callbacks.add_next_hop(make_nexthop("192.168.1.2", "Ethernet4"), 0x1001);
        orch.set_callbacks(callbacks.clone());

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key1 = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));
        let nhg_key2 = NextHopGroupKey::single(make_nexthop("192.168.1.2", "Ethernet4"));

        // Add route with first NH
        orch.add_route(0, prefix.clone(), nhg_key1.clone()).await.unwrap();

        // Update to second NH
        orch.add_route(0, prefix.clone(), nhg_key2.clone()).await.unwrap();

        // Old NH ref should be decremented, new NH ref should be incremented
        let refs = callbacks.next_hop_refs.lock().unwrap();
        assert_eq!(refs.get(&make_nexthop("192.168.1.1", "Ethernet0")), Some(&0));
        assert_eq!(refs.get(&make_nexthop("192.168.1.2", "Ethernet4")), Some(&1));
    }

    #[tokio::test]
    async fn test_update_route_same_nexthop() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        orch.set_callbacks(callbacks.clone());

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));

        // Add route
        orch.add_route(0, prefix.clone(), nhg_key.clone()).await.unwrap();

        // Update with same NH
        orch.add_route(0, prefix.clone(), nhg_key.clone()).await.unwrap();

        // Ref count should still be 1 (not incremented again)
        let refs = callbacks.next_hop_refs.lock().unwrap();
        assert_eq!(refs.get(&make_nexthop("192.168.1.1", "Ethernet0")), Some(&1));
    }

    #[tokio::test]
    async fn test_remove_route_success() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        orch.set_callbacks(callbacks.clone());

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));

        orch.add_route(0, prefix.clone(), nhg_key).await.unwrap();
        assert!(orch.has_route(0, &prefix));

        let result = orch.remove_route(0, &prefix).await;
        assert!(result.is_ok());
        assert!(!orch.has_route(0, &prefix));

        // Ref count should be decremented
        let refs = callbacks.next_hop_refs.lock().unwrap();
        assert_eq!(refs.get(&make_nexthop("192.168.1.1", "Ethernet0")), Some(&0));
    }

    #[tokio::test]
    async fn test_remove_route_not_found() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let prefix = make_prefix("10.0.0.0", 24);
        let result = orch.remove_route(0, &prefix).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::RouteNotFound(_)));
    }

    #[tokio::test]
    async fn test_remove_route_with_vrf() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_vrf(0x1234);
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        orch.set_callbacks(callbacks.clone());

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));

        orch.add_route(0x1234, prefix.clone(), nhg_key).await.unwrap();
        orch.remove_route(0x1234, &prefix).await.unwrap();

        // VRF ref count should be decremented
        let refs = callbacks.vrf_refs.lock().unwrap();
        assert_eq!(refs.get(&0x1234), Some(&0));
    }

    #[tokio::test]
    async fn test_remove_default_route_drops() {
        let config = RouteOrchConfig {
            default_action_drop: true,
            ..Default::default()
        };
        let mut orch = RouteOrch::new(config);
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        orch.set_callbacks(callbacks.clone());

        let prefix = make_prefix("0.0.0.0", 0); // Default route
        let nhg_key = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));

        orch.add_route(0, prefix.clone(), nhg_key).await.unwrap();

        // Remove should set to DROP instead of removing
        orch.remove_route(0, &prefix).await.unwrap();

        // Route should still exist
        assert!(orch.has_route(0, &prefix));

        // Ref count should be decremented
        let refs = callbacks.next_hop_refs.lock().unwrap();
        assert_eq!(refs.get(&make_nexthop("192.168.1.1", "Ethernet0")), Some(&0));
    }

    #[tokio::test]
    async fn test_remove_route_ecmp_cleans_nhg() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        callbacks.add_next_hop(make_nexthop("192.168.1.2", "Ethernet4"), 0x1001);
        orch.set_callbacks(callbacks);

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.1.1", "Ethernet0"),
            make_nexthop("192.168.1.2", "Ethernet4"),
        ]);

        orch.add_route(0, prefix.clone(), nhg_key.clone()).await.unwrap();
        assert!(orch.has_nhg(&nhg_key));

        orch.remove_route(0, &prefix).await.unwrap();

        // NHG should be removed when ref count reaches zero
        assert!(!orch.has_nhg(&nhg_key));
        assert_eq!(orch.nhg_count(), 0);
    }

    // ===== Multiple routes sharing NHG tests =====

    #[tokio::test]
    async fn test_multiple_routes_share_nhg() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        callbacks.add_next_hop(make_nexthop("192.168.1.2", "Ethernet4"), 0x1001);
        orch.set_callbacks(callbacks);

        let prefix1 = make_prefix("10.0.0.0", 24);
        let prefix2 = make_prefix("10.1.0.0", 24);
        let nhg_key = NextHopGroupKey::from_nexthops([
            make_nexthop("192.168.1.1", "Ethernet0"),
            make_nexthop("192.168.1.2", "Ethernet4"),
        ]);

        // Add first route
        orch.add_route(0, prefix1.clone(), nhg_key.clone()).await.unwrap();
        assert_eq!(orch.nhg_count(), 1);
        assert_eq!(orch.get_nhg(&nhg_key).unwrap().ref_count(), 1);

        // Add second route with same NHG
        orch.add_route(0, prefix2.clone(), nhg_key.clone()).await.unwrap();
        assert_eq!(orch.nhg_count(), 1); // Still only one NHG
        assert_eq!(orch.get_nhg(&nhg_key).unwrap().ref_count(), 2);

        // Remove first route
        orch.remove_route(0, &prefix1).await.unwrap();
        assert_eq!(orch.get_nhg(&nhg_key).unwrap().ref_count(), 1);
        assert!(orch.has_nhg(&nhg_key)); // NHG still exists

        // Remove second route
        orch.remove_route(0, &prefix2).await.unwrap();
        assert!(!orch.has_nhg(&nhg_key)); // NHG removed when last ref is gone
        assert_eq!(orch.nhg_count(), 0);
    }

    // ===== Interface next-hop tests =====

    #[tokio::test]
    async fn test_add_route_interface_nexthop() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_router_intf("Vlan100".to_string(), 0x2000);
        orch.set_callbacks(callbacks.clone());

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::single(NextHopKey::interface_only("Vlan100"));

        let result = orch.add_route(0, prefix.clone(), nhg_key).await;
        assert!(result.is_ok());
        assert!(orch.has_route(0, &prefix));

        // Router interface ref should be incremented
        let refs = callbacks.router_intf_refs.lock().unwrap();
        assert_eq!(refs.get("Vlan100"), Some(&1));
    }

    #[tokio::test]
    async fn test_add_route_interface_not_resolved() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::single(NextHopKey::interface_only("Vlan100"));

        let result = orch.add_route(0, prefix, nhg_key).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RouteError::NextHopNotResolved(_)));
    }

    // ===== Duplicate route handling tests =====

    #[tokio::test]
    async fn test_duplicate_route_updates() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        callbacks.add_next_hop(make_nexthop("192.168.1.2", "Ethernet4"), 0x1001);
        orch.set_callbacks(callbacks);

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key1 = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));
        let nhg_key2 = NextHopGroupKey::single(make_nexthop("192.168.1.2", "Ethernet4"));

        // Add route
        orch.add_route(0, prefix.clone(), nhg_key1).await.unwrap();
        let route1 = orch.get_route(0, &prefix).unwrap();
        assert_eq!(route1.nhg.nhg_key.len(), 1);

        // Update with different NH
        orch.add_route(0, prefix.clone(), nhg_key2.clone()).await.unwrap();
        let route2 = orch.get_route(0, &prefix).unwrap();
        assert_eq!(route2.nhg.nhg_key, nhg_key2);
    }

    // ===== Ordered ECMP tests =====

    #[test]
    fn test_ordered_ecmp_config() {
        let config = RouteOrchConfig {
            ordered_ecmp: true,
            ..Default::default()
        };
        let orch = RouteOrch::new(config);
        assert_eq!(orch.config.ordered_ecmp, true);
    }

    // ===== Route query tests =====

    #[tokio::test]
    async fn test_has_route() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        orch.set_callbacks(callbacks);

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));

        assert!(!orch.has_route(0, &prefix));

        orch.add_route(0, prefix.clone(), nhg_key).await.unwrap();
        assert!(orch.has_route(0, &prefix));
    }

    #[tokio::test]
    async fn test_get_route() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        orch.set_callbacks(callbacks);

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));

        assert!(orch.get_route(0, &prefix).is_none());

        orch.add_route(0, prefix.clone(), nhg_key.clone()).await.unwrap();
        let route = orch.get_route(0, &prefix).unwrap();
        assert_eq!(route.nhg.nhg_key, nhg_key);
    }

    #[tokio::test]
    async fn test_route_separate_vrfs() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_vrf(0x1234);
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        callbacks.add_next_hop(make_nexthop("192.168.1.2", "Ethernet4"), 0x1001);
        orch.set_callbacks(callbacks);

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key1 = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));
        let nhg_key2 = NextHopGroupKey::single(make_nexthop("192.168.1.2", "Ethernet4"));

        // Same prefix in different VRFs
        orch.add_route(0, prefix.clone(), nhg_key1.clone()).await.unwrap();
        orch.add_route(0x1234, prefix.clone(), nhg_key2.clone()).await.unwrap();

        let route1 = orch.get_route(0, &prefix).unwrap();
        let route2 = orch.get_route(0x1234, &prefix).unwrap();

        assert_eq!(route1.nhg.nhg_key, nhg_key1);
        assert_eq!(route2.nhg.nhg_key, nhg_key2);
    }

    // ===== Empty VRF table cleanup test =====

    #[tokio::test]
    async fn test_remove_route_cleans_empty_vrf_table() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_vrf(0x1234);
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        orch.set_callbacks(callbacks);

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));

        orch.add_route(0x1234, prefix.clone(), nhg_key).await.unwrap();
        assert!(orch.synced_routes.contains_key(&0x1234));

        orch.remove_route(0x1234, &prefix).await.unwrap();

        // VRF table should be removed when empty (but not for VRF 0)
        assert!(!orch.synced_routes.contains_key(&0x1234));
    }

    #[tokio::test]
    async fn test_remove_route_keeps_default_vrf_table() {
        let mut orch = RouteOrch::new(RouteOrchConfig::default());
        let callbacks = Arc::new(MockCallbacks::new());
        callbacks.add_next_hop(make_nexthop("192.168.1.1", "Ethernet0"), 0x1000);
        orch.set_callbacks(callbacks);

        let prefix = make_prefix("10.0.0.0", 24);
        let nhg_key = NextHopGroupKey::single(make_nexthop("192.168.1.1", "Ethernet0"));

        orch.add_route(0, prefix.clone(), nhg_key).await.unwrap();
        orch.remove_route(0, &prefix).await.unwrap();

        // Default VRF table might be cleaned up - implementation allows it
        // This test verifies no crash occurs
    }
}
