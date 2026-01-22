//! Fabric ports types and structures.

use sonic_sai::types::RawSaiObjectId;

/// Port isolation states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationState {
    Active,
    AutoIsolated,
    ConfigIsolated,
    PermIsolated,
}

/// Link health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    Up,
    Down,
}

/// Port health state (stub).
#[derive(Debug, Clone, Default)]
pub struct PortHealthState {
    pub consecutive_polls_with_errors: u64,
    pub consecutive_polls_with_no_errors: u64,
}

/// Fabric port state (stub).
#[derive(Debug, Clone)]
pub struct FabricPortState {
    pub lane: u32,
    pub sai_oid: RawSaiObjectId,
    pub status: LinkStatus,
    pub health: PortHealthState,
    pub isolation: IsolationState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolation_state() {
        assert_ne!(IsolationState::Active, IsolationState::AutoIsolated);
    }
}
