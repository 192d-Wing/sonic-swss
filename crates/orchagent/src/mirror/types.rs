//! Mirror session types and structures.

pub use sonic_sai::types::RawSaiObjectId;
use sonic_types::IpAddress;

/// Mirror session type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirrorSessionType {
    Span,
    Erspan,
}

/// Mirror traffic direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirrorDirection {
    Rx,
    Tx,
    Both,
}

/// Mirror session configuration (stub).
#[derive(Debug, Clone)]
pub struct MirrorSessionConfig {
    pub session_type: MirrorSessionType,
    pub direction: MirrorDirection,
    pub dst_port: Option<String>,
    pub src_ip: Option<IpAddress>,
    pub dst_ip: Option<IpAddress>,
}

/// Mirror session entry (stub).
#[derive(Debug, Clone)]
pub struct MirrorEntry {
    pub session_id: Option<RawSaiObjectId>,
    pub config: MirrorSessionConfig,
    pub ref_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_type() {
        assert_ne!(MirrorSessionType::Span, MirrorSessionType::Erspan);
    }
}
