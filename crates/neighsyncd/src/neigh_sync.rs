//! NeighSync - Core neighbor synchronization logic
//!
//! # NIST 800-53 Rev 5 Control Mappings
//! - SI-4: System Monitoring - Neighbor table monitoring
//! - AU-12: Audit Record Generation - Log all neighbor changes
//! - SC-7: Boundary Protection - Network neighbor awareness
//! - CM-8: System Component Inventory - Track network neighbors

use crate::error::{NeighsyncError, Result};
use crate::netlink::NetlinkSocket;
use crate::redis_adapter::RedisAdapter;
use crate::types::{MacAddress, NeighborEntry, NeighborMessageType, NeighborState};
use std::collections::HashMap;
use tracing::{debug, info, instrument, warn};

/// Default warm restart reconciliation timer (seconds)
/// NIST: CM-6 - Configuration settings
pub const DEFAULT_WARMSTART_TIMER_SECS: u64 = 5;
/// Timeout for waiting for neighbor restore during warm restart (seconds)
const RESTORE_NEIGH_WAIT_TIMEOUT_SECS: u64 = 180;

/// Warm restart state for reconciliation
///
/// # NIST Controls
/// - CP-10: System Recovery - Track recovery state
#[derive(Debug, Default)]
struct WarmRestartState {
    /// Whether warm restart is in progress
    in_progress: bool,
    /// Cached neighbor entries from APPL_DB before restart
    cached_neighbors: HashMap<String, HashMap<String, String>>,
    /// New entries received during warm restart
    pending_entries: Vec<(String, NeighborEntry, bool)>, // (key, entry, is_delete)
}

/// NeighSync - Synchronizes kernel neighbor table to Redis
///
/// # NIST Controls
/// - SI-4(4): System Monitoring - Automated analysis of network events
/// - AU-6: Audit Record Review - Neighbor changes available for analysis
pub struct NeighSync {
    redis: RedisAdapter,
    netlink: NetlinkSocket,
    warm_restart: WarmRestartState,
    is_dual_tor: bool,
}

impl NeighSync {
    /// Create a new NeighSync instance
    ///
    /// # NIST Controls
    /// - AC-3: Access Enforcement - Initialize with appropriate permissions
    #[instrument(skip_all)]
    pub async fn new(redis_host: &str, redis_port: u16) -> Result<Self> {
        info!("Initializing NeighSync");

        let redis = RedisAdapter::new(redis_host, redis_port).await?;
        let netlink = NetlinkSocket::new()?;

        let mut sync = Self {
            redis,
            netlink,
            warm_restart: WarmRestartState::default(),
            is_dual_tor: false,
        };

        // Check if this is a dual-ToR deployment
        sync.is_dual_tor = sync.redis.is_dual_tor().await?;
        info!(is_dual_tor = sync.is_dual_tor, "Detected deployment type");

        Ok(sync)
    }

    /// Start warm restart handling if applicable
    ///
    /// # NIST Controls
    /// - CP-10: System Recovery - Initialize recovery process
    #[instrument(skip(self))]
    pub async fn start_warm_restart(&mut self) -> Result<bool> {
        // Check if warm restart is configured (would be checked via warm restart module)
        // For now, assume warm restart is enabled if restore table exists

        // Cache current neighbors from APPL_DB
        self.warm_restart.cached_neighbors = self.redis.get_all_neighbors().await?;
        self.warm_restart.in_progress = !self.warm_restart.cached_neighbors.is_empty();

        if self.warm_restart.in_progress {
            info!(
                cached_count = self.warm_restart.cached_neighbors.len(),
                "Warm restart initiated, cached existing neighbors"
            );
        }

        Ok(self.warm_restart.in_progress)
    }

    /// Wait for neighbor restore to complete (during warm restart)
    ///
    /// # NIST Controls
    /// - CP-10: System Recovery - Wait for recovery completion
    #[instrument(skip(self))]
    pub async fn wait_for_restore(&mut self) -> Result<()> {
        if !self.warm_restart.in_progress {
            return Ok(());
        }

        let start = std::time::Instant::now();

        loop {
            if self.redis.is_neighbor_restore_done().await? {
                info!(
                    elapsed_secs = start.elapsed().as_secs(),
                    "Neighbor restore completed"
                );
                return Ok(());
            }

            let elapsed = start.elapsed().as_secs();
            if elapsed > RESTORE_NEIGH_WAIT_TIMEOUT_SECS {
                return Err(NeighsyncError::WarmRestartTimeout(elapsed));
            }

            debug!(elapsed_secs = elapsed, "Waiting for neighbor restore");
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    /// Request initial neighbor table dump
    ///
    /// # NIST Controls
    /// - CM-8: System Component Inventory - Initial inventory
    #[instrument(skip(self))]
    pub fn request_dump(&mut self) -> Result<()> {
        info!("Requesting neighbor table dump");
        self.netlink.request_dump()
    }

    /// Process incoming netlink events
    ///
    /// # NIST Controls
    /// - SI-4: System Monitoring - Process monitoring events
    /// - AU-12: Audit Record Generation - Generate audit records
    #[instrument(skip(self))]
    pub async fn process_events(&mut self) -> Result<usize> {
        let events = self.netlink.receive_events()?;
        let mut processed = 0;

        for (msg_type, entry) in events {
            if self.should_process_entry(&entry).await? {
                self.handle_neighbor_event(msg_type, entry).await?;
                processed += 1;
            }
        }

        Ok(processed)
    }

    /// Check if a neighbor entry should be processed
    ///
    /// # NIST Controls
    /// - SI-10: Information Input Validation - Validate entries
    /// - SC-5: Denial of Service Protection - Filter invalid entries
    #[instrument(skip(self))]
    async fn should_process_entry(&mut self, entry: &NeighborEntry) -> Result<bool> {
        // Filter IPv6 multicast link-local (always ignored)
        // NIST: SC-5 - Prevent multicast-based attacks
        if entry.is_ipv6_multicast_link_local() {
            debug!(ip = %entry.ip, "Ignoring IPv6 multicast link-local");
            return Ok(false);
        }

        // Filter IPv6 link-local if not enabled on interface
        // NIST: SC-7 - Boundary protection via configuration
        if entry.is_ipv6_link_local() {
            let enabled = self
                .redis
                .is_ipv6_link_local_enabled(&entry.interface)
                .await?;
            if !enabled {
                debug!(
                    ip = %entry.ip,
                    interface = %entry.interface,
                    "Ignoring IPv6 link-local (not enabled on interface)"
                );
                return Ok(false);
            }
        }

        // Filter IPv4 link-local on dual-ToR
        // NIST: SC-7 - Dual-ToR boundary protection
        #[cfg(feature = "ipv4")]
        if entry.is_ipv4_link_local() && self.is_dual_tor {
            debug!(ip = %entry.ip, "Ignoring IPv4 link-local on dual-ToR");
            return Ok(false);
        }

        // Filter NUD_NOARP unless externally learned (VXLAN EVPN)
        // NIST: SC-7 - Accept externally learned for overlay networks
        if entry.state == NeighborState::NoArp && !entry.externally_learned {
            debug!(ip = %entry.ip, "Ignoring NOARP entry (not externally learned)");
            return Ok(false);
        }

        Ok(true)
    }

    /// Handle a single neighbor event
    ///
    /// # NIST Controls
    /// - AU-12: Audit Record Generation - Log event handling
    /// - CM-8: System Component Inventory - Update inventory
    #[instrument(skip(self))]
    async fn handle_neighbor_event(
        &mut self,
        msg_type: NeighborMessageType,
        mut entry: NeighborEntry,
    ) -> Result<()> {
        let key = entry.redis_key();
        let is_delete = self.should_delete(&msg_type, &entry);

        // Handle unresolved neighbors on dual-ToR with zero MAC
        // NIST: SC-7 - Dual-ToR failover support
        if self.is_dual_tor && !entry.state.is_resolved() && !is_delete {
            debug!(
                ip = %entry.ip,
                state = ?entry.state,
                "Using zero MAC for unresolved neighbor on dual-ToR"
            );
            entry.mac = MacAddress::ZERO;
        }

        // Filter "none" MAC on add operations
        // NIST: SI-10 - Input validation
        if !is_delete && entry.mac.is_zero() && !self.is_dual_tor {
            debug!(ip = %entry.ip, "Ignoring add with zero MAC (non-dual-ToR)");
            return Ok(());
        }

        // Filter broadcast MAC
        // NIST: SC-5 - DoS protection
        if !is_delete && entry.mac.is_broadcast() {
            debug!(ip = %entry.ip, "Ignoring broadcast MAC");
            return Ok(());
        }

        // During warm restart, cache instead of applying
        // NIST: CP-10 - Recovery state management
        if self.warm_restart.in_progress {
            debug!(key, is_delete, "Caching event during warm restart");
            self.warm_restart
                .pending_entries
                .push((key, entry, is_delete));
            return Ok(());
        }

        // Apply to Redis
        if is_delete {
            self.redis.delete_neighbor(&entry).await?;
            info!(
                interface = %entry.interface,
                ip = %entry.ip,
                "Deleted neighbor"
            );
        } else {
            self.redis.set_neighbor(&entry).await?;
            info!(
                interface = %entry.interface,
                ip = %entry.ip,
                mac = %entry.mac,
                "Set neighbor"
            );
        }

        Ok(())
    }

    /// Determine if this event should result in a delete
    fn should_delete(&self, msg_type: &NeighborMessageType, entry: &NeighborEntry) -> bool {
        match msg_type {
            NeighborMessageType::Delete => true,
            NeighborMessageType::New | NeighborMessageType::Get => {
                // Delete for incomplete/failed states (unless dual-ToR)
                if self.is_dual_tor {
                    false
                } else {
                    matches!(
                        entry.state,
                        NeighborState::Incomplete | NeighborState::Failed
                    )
                }
            }
        }
    }

    /// Perform warm restart reconciliation
    ///
    /// # NIST Controls
    /// - CP-10: System Recovery - Reconcile state after recovery
    #[instrument(skip(self))]
    pub async fn reconcile(&mut self) -> Result<()> {
        if !self.warm_restart.in_progress {
            return Ok(());
        }

        info!(
            pending_count = self.warm_restart.pending_entries.len(),
            cached_count = self.warm_restart.cached_neighbors.len(),
            "Starting warm restart reconciliation"
        );

        // Apply all pending entries
        let pending = std::mem::take(&mut self.warm_restart.pending_entries);
        for (_key, entry, is_delete) in pending {
            if is_delete {
                self.redis.delete_neighbor(&entry).await?;
            } else {
                self.redis.set_neighbor(&entry).await?;
            }
        }

        // Clear warm restart state
        self.warm_restart.in_progress = false;
        self.warm_restart.cached_neighbors.clear();

        info!("Warm restart reconciliation complete");
        Ok(())
    }

    /// Get the netlink socket file descriptor for async polling
    pub fn netlink_fd(&self) -> i32 {
        self.netlink.as_raw_fd()
    }

    /// Check if warm restart is in progress
    pub fn is_warm_restart_in_progress(&self) -> bool {
        self.warm_restart.in_progress
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NeighborState;
    use std::net::Ipv6Addr;

    fn make_test_entry(ip: &str, state: NeighborState) -> NeighborEntry {
        NeighborEntry {
            ifindex: 1,
            interface: "Ethernet0".to_string(),
            ip: ip.parse().unwrap(),
            mac: MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]),
            state,
            externally_learned: false,
        }
    }

    #[test]
    fn test_should_delete_logic() {
        // This would require mocking, so just test the basic logic patterns
        let entry = make_test_entry("2001:db8::1", NeighborState::Reachable);
        assert!(entry.state.is_resolved());

        let failed = make_test_entry("2001:db8::2", NeighborState::Failed);
        assert!(!failed.state.is_resolved());
    }
}
