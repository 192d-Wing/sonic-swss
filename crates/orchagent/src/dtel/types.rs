//! DTel (Data Plane Telemetry) types and structures.

use sonic_sai::types::RawSaiObjectId;
use std::sync::atomic::AtomicU64;

/// DTel event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DtelEventType {
    FlowState,
    FlowReportAllPackets,
    FlowTcpFlag,
    QueueReportThresholdBreach,
    QueueReportTailDrop,
    DropReport,
}

/// INT session configuration (stub).
#[derive(Debug, Clone)]
pub struct IntSessionConfig {
    pub session_id: String,
    pub collect_switch_id: bool,
    pub max_hop_count: u16,
}

/// INT session entry with atomic ref counting.
#[derive(Debug)]
pub struct IntSessionEntry {
    pub session_oid: RawSaiObjectId,
    pub config: IntSessionConfig,
    pub ref_count: AtomicU64,
}

impl IntSessionEntry {
    pub fn new(session_oid: RawSaiObjectId, config: IntSessionConfig) -> Self {
        Self {
            session_oid,
            config,
            ref_count: AtomicU64::new(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type() {
        assert_ne!(DtelEventType::FlowState, DtelEventType::DropReport);
    }
}
