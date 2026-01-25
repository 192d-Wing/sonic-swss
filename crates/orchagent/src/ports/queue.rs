//! Queue and scheduler types for PortsOrch.
//!
//! This module defines queue and scheduler related types used in QoS configuration.

use sonic_sai::types::RawSaiObjectId;
use std::fmt;

/// Queue type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueueType {
    /// Unicast queue.
    #[default]
    Unicast,
    /// Multicast queue.
    Multicast,
    /// All (both unicast and multicast).
    All,
}

impl fmt::Display for QueueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unicast => write!(f, "UC"),
            Self::Multicast => write!(f, "MC"),
            Self::All => write!(f, "ALL"),
        }
    }
}

impl std::str::FromStr for QueueType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "UC" | "UNICAST" => Ok(Self::Unicast),
            "MC" | "MULTICAST" => Ok(Self::Multicast),
            "ALL" => Ok(Self::All),
            _ => Err(format!("Unknown queue type: {}", s)),
        }
    }
}

/// Scheduler type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SchedulerType {
    /// Strict priority scheduling.
    #[default]
    Strict,
    /// Weighted Round Robin.
    Wrr,
    /// Deficit Weighted Round Robin.
    Dwrr,
}

impl fmt::Display for SchedulerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Strict => write!(f, "STRICT"),
            Self::Wrr => write!(f, "WRR"),
            Self::Dwrr => write!(f, "DWRR"),
        }
    }
}

impl std::str::FromStr for SchedulerType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "STRICT" => Ok(Self::Strict),
            "WRR" => Ok(Self::Wrr),
            "DWRR" => Ok(Self::Dwrr),
            _ => Err(format!("Unknown scheduler type: {}", s)),
        }
    }
}

/// Queue information structure.
#[derive(Debug, Clone)]
pub struct QueueInfo {
    /// SAI queue object ID.
    pub queue_id: RawSaiObjectId,
    /// Queue index (0-based within the port).
    pub index: u32,
    /// Queue type (unicast/multicast).
    pub queue_type: QueueType,
    /// Parent scheduler group ID.
    pub scheduler_group_id: Option<RawSaiObjectId>,
    /// WRED profile ID (if configured).
    pub wred_id: Option<RawSaiObjectId>,
    /// Buffer profile ID (if configured).
    pub buffer_profile_id: Option<RawSaiObjectId>,
    /// Scheduler profile ID (if configured).
    pub scheduler_id: Option<RawSaiObjectId>,
}

impl QueueInfo {
    /// Creates a new queue info entry.
    pub fn new(queue_id: RawSaiObjectId, index: u32, queue_type: QueueType) -> Self {
        Self {
            queue_id,
            index,
            queue_type,
            scheduler_group_id: None,
            wred_id: None,
            buffer_profile_id: None,
            scheduler_id: None,
        }
    }

    /// Returns the SAI queue ID.
    pub fn sai_id(&self) -> RawSaiObjectId {
        self.queue_id
    }
}

/// Scheduler information structure.
#[derive(Debug, Clone)]
pub struct SchedulerInfo {
    /// SAI scheduler object ID.
    pub scheduler_id: RawSaiObjectId,
    /// Scheduler type.
    pub scheduler_type: SchedulerType,
    /// Weight for WRR/DWRR scheduling.
    pub weight: u32,
    /// Minimum bandwidth (in bps).
    pub min_bandwidth_rate: u64,
    /// Maximum bandwidth (in bps).
    pub max_bandwidth_rate: u64,
    /// Minimum bandwidth in percentage.
    pub min_bandwidth_percent: u32,
    /// Maximum bandwidth in percentage.
    pub max_bandwidth_percent: u32,
}

impl SchedulerInfo {
    /// Creates a new scheduler info entry.
    pub fn new(scheduler_id: RawSaiObjectId, scheduler_type: SchedulerType) -> Self {
        Self {
            scheduler_id,
            scheduler_type,
            weight: 1,
            min_bandwidth_rate: 0,
            max_bandwidth_rate: 0,
            min_bandwidth_percent: 0,
            max_bandwidth_percent: 100,
        }
    }

    /// Returns the SAI scheduler ID.
    pub fn sai_id(&self) -> RawSaiObjectId {
        self.scheduler_id
    }

    /// Sets the weight.
    pub fn with_weight(mut self, weight: u32) -> Self {
        self.weight = weight;
        self
    }
}

/// Scheduler group information structure.
#[derive(Debug, Clone)]
pub struct SchedulerGroupInfo {
    /// SAI scheduler group object ID.
    pub scheduler_group_id: RawSaiObjectId,
    /// Level in the scheduler hierarchy.
    pub level: u32,
    /// Maximum number of children.
    pub max_childs: u32,
    /// Parent scheduler group ID (None for root).
    pub parent_id: Option<RawSaiObjectId>,
    /// Port ID this group belongs to.
    pub port_id: RawSaiObjectId,
    /// Child queue/group IDs.
    pub child_ids: Vec<RawSaiObjectId>,
}

impl SchedulerGroupInfo {
    /// Creates a new scheduler group info entry.
    pub fn new(scheduler_group_id: RawSaiObjectId, level: u32, port_id: RawSaiObjectId) -> Self {
        Self {
            scheduler_group_id,
            level,
            max_childs: 0,
            parent_id: None,
            port_id,
            child_ids: Vec::new(),
        }
    }

    /// Returns the SAI scheduler group ID.
    pub fn sai_id(&self) -> RawSaiObjectId {
        self.scheduler_group_id
    }

    /// Adds a child to this scheduler group.
    pub fn add_child(&mut self, child_id: RawSaiObjectId) {
        self.child_ids.push(child_id);
    }
}

/// Priority group information structure.
#[derive(Debug, Clone)]
pub struct PriorityGroupInfo {
    /// SAI priority group object ID.
    pub pg_id: RawSaiObjectId,
    /// Priority group index (0-7 typically).
    pub index: u32,
    /// Port ID this PG belongs to.
    pub port_id: RawSaiObjectId,
    /// Buffer profile ID (if configured).
    pub buffer_profile_id: Option<RawSaiObjectId>,
}

impl PriorityGroupInfo {
    /// Creates a new priority group info entry.
    pub fn new(pg_id: RawSaiObjectId, index: u32, port_id: RawSaiObjectId) -> Self {
        Self {
            pg_id,
            index,
            port_id,
            buffer_profile_id: None,
        }
    }

    /// Returns the SAI priority group ID.
    pub fn sai_id(&self) -> RawSaiObjectId {
        self.pg_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_type_parse() {
        assert_eq!("UC".parse::<QueueType>().unwrap(), QueueType::Unicast);
        assert_eq!(
            "MULTICAST".parse::<QueueType>().unwrap(),
            QueueType::Multicast
        );
        assert_eq!("ALL".parse::<QueueType>().unwrap(), QueueType::All);
    }

    #[test]
    fn test_scheduler_type_parse() {
        assert_eq!(
            "STRICT".parse::<SchedulerType>().unwrap(),
            SchedulerType::Strict
        );
        assert_eq!("WRR".parse::<SchedulerType>().unwrap(), SchedulerType::Wrr);
        assert_eq!(
            "DWRR".parse::<SchedulerType>().unwrap(),
            SchedulerType::Dwrr
        );
    }

    #[test]
    fn test_queue_info() {
        let queue = QueueInfo::new(0x1234, 0, QueueType::Unicast);
        assert_eq!(queue.sai_id(), 0x1234);
        assert_eq!(queue.index, 0);
        assert_eq!(queue.queue_type, QueueType::Unicast);
    }

    #[test]
    fn test_scheduler_info() {
        let scheduler = SchedulerInfo::new(0x5678, SchedulerType::Wrr).with_weight(10);
        assert_eq!(scheduler.sai_id(), 0x5678);
        assert_eq!(scheduler.scheduler_type, SchedulerType::Wrr);
        assert_eq!(scheduler.weight, 10);
    }

    #[test]
    fn test_scheduler_group_info() {
        let mut group = SchedulerGroupInfo::new(0xABCD, 0, 0x1111);
        assert_eq!(group.sai_id(), 0xABCD);
        assert_eq!(group.level, 0);
        assert_eq!(group.child_ids.len(), 0);

        group.add_child(0x2222);
        group.add_child(0x3333);
        assert_eq!(group.child_ids.len(), 2);
    }

    #[test]
    fn test_priority_group_info() {
        let pg = PriorityGroupInfo::new(0xDEAD, 3, 0xBEEF);
        assert_eq!(pg.sai_id(), 0xDEAD);
        assert_eq!(pg.index, 3);
        assert_eq!(pg.port_id, 0xBEEF);
    }
}
