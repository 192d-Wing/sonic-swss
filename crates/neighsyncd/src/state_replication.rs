//! State replication and synchronization for high availability
//!
//! Provides distributed state synchronization across multiple neighsyncd instances
//! for HA deployments. Ensures consistent neighbor state across the cluster.

use crate::error::{NeighsyncError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// Replication event types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicationEventType {
    /// Neighbor added/updated
    NeighborAdded,
    /// Neighbor deleted
    NeighborDeleted,
    /// State snapshot (periodic)
    StateSnapshot,
    /// Reconciliation request
    ReconciliationRequest,
    /// Heartbeat (liveness check)
    Heartbeat,
}

/// State replication message
#[derive(Debug, Clone)]
pub struct ReplicationMessage {
    /// Unique message ID for deduplication
    pub message_id: String,
    /// Source instance ID
    pub source_id: String,
    /// Target instance ID (None = broadcast)
    pub target_id: Option<String>,
    /// Event type
    pub event_type: ReplicationEventType,
    /// Timestamp (unix seconds)
    pub timestamp: u64,
    /// Sequence number for ordering
    pub sequence: u64,
    /// Payload (neighbor data or snapshot)
    pub payload: Vec<u8>,
}

impl ReplicationMessage {
    /// Create a new replication message
    pub fn new(source_id: String, event_type: ReplicationEventType, payload: Vec<u8>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            message_id: format!("{}-{}", source_id, timestamp),
            source_id,
            target_id: None,
            event_type,
            timestamp,
            sequence: 0,
            payload,
        }
    }

    /// Set target instance
    pub fn with_target(mut self, target_id: String) -> Self {
        self.target_id = Some(target_id);
        self
    }

    /// Set sequence number
    pub fn with_sequence(mut self, seq: u64) -> Self {
        self.sequence = seq;
        self
    }
}

/// Replication state tracker
#[derive(Debug, Clone)]
pub struct ReplicationState {
    /// Instance ID for this daemon
    pub instance_id: String,
    /// Last processed sequence number
    pub last_sequence: u64,
    /// Last replication timestamp
    pub last_replication_time: u64,
    /// Number of replications sent
    pub replications_sent: u64,
    /// Number of replications received
    pub replications_received: u64,
}

impl ReplicationState {
    /// Create new replication state
    pub fn new(instance_id: String) -> Self {
        Self {
            instance_id,
            last_sequence: 0,
            last_replication_time: 0,
            replications_sent: 0,
            replications_received: 0,
        }
    }

    /// Record sent replication
    pub fn record_sent(&mut self) {
        self.replications_sent += 1;
        self.last_replication_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// Record received replication
    pub fn record_received(&mut self, sequence: u64) {
        self.replications_received += 1;
        self.last_sequence = self.last_sequence.max(sequence);
        self.last_replication_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

/// Replication manager for state synchronization
pub struct ReplicationManager {
    /// Local instance ID
    instance_id: String,
    /// Replication state
    state: Arc<parking_lot::Mutex<ReplicationState>>,
    /// Remote instances and their states
    remote_instances: Arc<parking_lot::Mutex<HashMap<String, RemoteInstance>>>,
    /// Message sequence counter
    sequence_counter: Arc<AtomicU64>,
    /// Processed message IDs for deduplication
    processed_messages: Arc<parking_lot::Mutex<std::collections::HashSet<String>>>,
}

/// Remote instance information
#[derive(Debug, Clone)]
pub struct RemoteInstance {
    /// Instance ID
    pub id: String,
    /// Last heartbeat timestamp
    pub last_heartbeat: u64,
    /// Acknowledged sequence number
    pub acked_sequence: u64,
    /// Health status
    pub is_healthy: bool,
}

impl ReplicationManager {
    /// Create new replication manager
    pub fn new(instance_id: String) -> Self {
        Self {
            instance_id: instance_id.clone(),
            state: Arc::new(parking_lot::Mutex::new(ReplicationState::new(instance_id))),
            remote_instances: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            sequence_counter: Arc::new(AtomicU64::new(0)),
            processed_messages: Arc::new(parking_lot::Mutex::new(std::collections::HashSet::new())),
        }
    }

    /// Register remote instance
    pub fn register_remote(&self, instance_id: String) -> Result<()> {
        let mut remotes = self.remote_instances.lock();
        if remotes.contains_key(&instance_id) {
            return Err(NeighsyncError::Replication(format!(
                "Instance {} already registered",
                instance_id
            )));
        }

        remotes.insert(
            instance_id.clone(),
            RemoteInstance {
                id: instance_id.clone(),
                last_heartbeat: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                acked_sequence: 0,
                is_healthy: true,
            },
        );

        info!(instance = %instance_id, "Registered remote instance");
        Ok(())
    }

    /// Update remote instance heartbeat
    pub fn update_heartbeat(&self, instance_id: &str) -> Result<()> {
        let mut remotes = self.remote_instances.lock();
        match remotes.get_mut(instance_id) {
            Some(remote) => {
                remote.last_heartbeat = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                remote.is_healthy = true;
                debug!(instance = %instance_id, "Updated heartbeat");
                Ok(())
            }
            None => Err(NeighsyncError::Replication(format!(
                "Instance {} not registered",
                instance_id
            ))),
        }
    }

    /// Create a new replication message
    pub fn create_message(
        &self,
        event_type: ReplicationEventType,
        payload: Vec<u8>,
    ) -> ReplicationMessage {
        let sequence = self.sequence_counter.fetch_add(1, Ordering::SeqCst);
        let mut msg = ReplicationMessage::new(self.instance_id.clone(), event_type, payload);
        msg.sequence = sequence;
        msg
    }

    /// Process received message (with deduplication)
    pub fn process_message(&self, msg: &ReplicationMessage) -> Result<bool> {
        // Check for duplicate
        let mut processed = self.processed_messages.lock();
        if processed.contains(&msg.message_id) {
            debug!(msg_id = %msg.message_id, "Duplicate message ignored");
            return Ok(false);
        }

        // Limit cache size to prevent unbounded growth
        if processed.len() > 10000 {
            processed.clear();
        }

        processed.insert(msg.message_id.clone());

        // Update state
        let mut state = self.state.lock();
        state.record_received(msg.sequence);

        debug!(
            source = %msg.source_id,
            seq = msg.sequence,
            event = ?msg.event_type,
            "Processed replication message"
        );

        Ok(true)
    }

    /// Check instance health (heartbeat timeout = 30s)
    pub fn check_instance_health(&self, instance_id: &str, timeout_secs: u64) -> Result<bool> {
        let mut remotes = self.remote_instances.lock();
        match remotes.get_mut(instance_id) {
            Some(remote) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let is_healthy = (now - remote.last_heartbeat) < timeout_secs;
                let was_healthy = remote.is_healthy;

                remote.is_healthy = is_healthy;

                if was_healthy && !is_healthy {
                    warn!(
                        instance = %instance_id,
                        timeout = timeout_secs,
                        "Instance health check failed"
                    );
                } else if !was_healthy && is_healthy {
                    info!(instance = %instance_id, "Instance recovered");
                }

                Ok(is_healthy)
            }
            None => Err(NeighsyncError::Replication(format!(
                "Instance {} not registered",
                instance_id
            ))),
        }
    }

    /// Get replication statistics
    pub fn stats(&self) -> (u64, u64, u64) {
        let state = self.state.lock();
        (
            state.replications_sent,
            state.replications_received,
            state.last_sequence,
        )
    }

    /// Get healthy remote instances
    pub fn get_healthy_remotes(&self) -> Vec<String> {
        let remotes = self.remote_instances.lock();
        remotes
            .values()
            .filter(|r| r.is_healthy)
            .map(|r| r.id.clone())
            .collect()
    }

    /// Get all registered instances
    pub fn get_all_instances(&self) -> Vec<String> {
        let remotes = self.remote_instances.lock();
        remotes.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replication_message_creation() {
        let msg = ReplicationMessage::new(
            "node1".to_string(),
            ReplicationEventType::NeighborAdded,
            vec![1, 2, 3],
        );

        assert_eq!(msg.source_id, "node1");
        assert_eq!(msg.event_type, ReplicationEventType::NeighborAdded);
        assert_eq!(msg.payload, vec![1, 2, 3]);
        assert!(msg.timestamp > 0);
    }

    #[test]
    fn test_replication_message_with_target() {
        let msg =
            ReplicationMessage::new("node1".to_string(), ReplicationEventType::Heartbeat, vec![])
                .with_target("node2".to_string());

        assert_eq!(msg.target_id, Some("node2".to_string()));
    }

    #[test]
    fn test_replication_state() {
        let mut state = ReplicationState::new("node1".to_string());
        assert_eq!(state.replications_sent, 0);

        state.record_sent();
        assert_eq!(state.replications_sent, 1);
        assert!(state.last_replication_time > 0);

        state.record_received(5);
        assert_eq!(state.replications_received, 1);
        assert_eq!(state.last_sequence, 5);
    }

    #[test]
    fn test_replication_manager_creation() {
        let manager = ReplicationManager::new("node1".to_string());
        let (sent, recv, seq) = manager.stats();
        assert_eq!(sent, 0);
        assert_eq!(recv, 0);
        assert_eq!(seq, 0);
    }

    #[test]
    fn test_register_remote_instance() {
        let manager = ReplicationManager::new("node1".to_string());
        assert!(manager.register_remote("node2".to_string()).is_ok());

        // Should fail to re-register
        assert!(manager.register_remote("node2".to_string()).is_err());
    }

    #[test]
    fn test_message_deduplication() {
        let manager = ReplicationManager::new("node1".to_string());
        let msg =
            ReplicationMessage::new("node2".to_string(), ReplicationEventType::Heartbeat, vec![]);

        // First receive should succeed
        assert!(manager.process_message(&msg).unwrap());

        // Duplicate should be ignored
        assert!(!manager.process_message(&msg).unwrap());
    }

    #[test]
    fn test_sequence_number_increment() {
        let manager = ReplicationManager::new("node1".to_string());
        let msg1 = manager.create_message(ReplicationEventType::NeighborAdded, vec![]);
        let msg2 = manager.create_message(ReplicationEventType::NeighborAdded, vec![]);
        let msg3 = manager.create_message(ReplicationEventType::NeighborAdded, vec![]);

        assert_eq!(msg1.sequence, 0);
        assert_eq!(msg2.sequence, 1);
        assert_eq!(msg3.sequence, 2);
    }

    #[test]
    fn test_heartbeat_tracking() {
        let manager = ReplicationManager::new("node1".to_string());
        manager.register_remote("node2".to_string()).unwrap();

        // Should be healthy initially
        assert!(manager.check_instance_health("node2", 30).unwrap());

        // Update heartbeat
        manager.update_heartbeat("node2").unwrap();
        assert!(manager.check_instance_health("node2", 30).unwrap());
    }

    #[test]
    fn test_get_healthy_remotes() {
        let manager = ReplicationManager::new("node1".to_string());
        manager.register_remote("node2".to_string()).unwrap();
        manager.register_remote("node3".to_string()).unwrap();

        let healthy = manager.get_healthy_remotes();
        assert_eq!(healthy.len(), 2);
    }
}
