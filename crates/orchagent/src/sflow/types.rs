//! SflowOrch types.

use sonic_sai::types::RawSaiObjectId;
use std::num::NonZeroU32;

/// Sflow sampling direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleDirection {
    /// Sample received (ingress) packets.
    Rx,
    /// Sample transmitted (egress) packets.
    Tx,
    /// Sample both ingress and egress packets.
    Both,
}

impl SampleDirection {
    /// Parses a direction string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "rx" => Some(Self::Rx),
            "tx" => Some(Self::Tx),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    /// Returns the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rx => "rx",
            Self::Tx => "tx",
            Self::Both => "both",
        }
    }

    /// Returns true if this direction includes ingress sampling.
    pub fn has_ingress(&self) -> bool {
        matches!(self, Self::Rx | Self::Both)
    }

    /// Returns true if this direction includes egress sampling.
    pub fn has_egress(&self) -> bool {
        matches!(self, Self::Tx | Self::Both)
    }
}

/// Port sflow configuration.
#[derive(Debug, Clone)]
pub struct PortSflowInfo {
    /// Whether sflow is administratively enabled on this port.
    pub admin_state: bool,
    /// Sampling direction.
    pub direction: SampleDirection,
    /// SAI sample session ID associated with this port.
    pub session_id: RawSaiObjectId,
}

impl PortSflowInfo {
    /// Creates a new port sflow info.
    pub fn new(admin_state: bool, direction: SampleDirection, session_id: RawSaiObjectId) -> Self {
        Self {
            admin_state,
            direction,
            session_id,
        }
    }
}

/// Sflow session (shared by multiple ports at the same sample rate).
#[derive(Debug, Clone)]
pub struct SflowSession {
    /// SAI samplepacket object ID.
    pub session_id: RawSaiObjectId,
    /// Sample rate (1 in N packets).
    pub rate: NonZeroU32,
    /// Number of ports currently using this session.
    pub ref_count: u32,
}

impl SflowSession {
    /// Creates a new sflow session.
    pub fn new(session_id: RawSaiObjectId, rate: NonZeroU32) -> Self {
        Self {
            session_id,
            rate,
            ref_count: 0,
        }
    }

    /// Increments the reference count.
    pub fn add_ref(&mut self) {
        self.ref_count += 1;
    }

    /// Decrements the reference count.
    pub fn remove_ref(&mut self) -> u32 {
        self.ref_count = self.ref_count.saturating_sub(1);
        self.ref_count
    }
}

/// Sflow configuration parsed from field-value tuples.
#[derive(Debug, Clone)]
pub struct SflowConfig {
    /// Administrative state.
    pub admin_state: bool,
    /// Sample rate (None means no change).
    pub rate: Option<NonZeroU32>,
    /// Sample direction.
    pub direction: SampleDirection,
}

impl SflowConfig {
    /// Creates a new config with defaults.
    pub fn new() -> Self {
        Self {
            admin_state: false,
            rate: None,
            direction: SampleDirection::Rx,
        }
    }

    /// Parses a field-value pair and updates the config.
    pub fn parse_field(&mut self, field: &str, value: &str) -> Result<(), String> {
        match field {
            "admin_state" => {
                self.admin_state = match value {
                    "up" => true,
                    "down" => false,
                    _ => return Err(format!("Invalid admin_state: {}", value)),
                };
            }
            "sample_rate" => {
                if value == "error" {
                    self.rate = None;
                } else {
                    let rate = value
                        .parse::<u32>()
                        .map_err(|e| format!("Invalid sample_rate '{}': {}", value, e))?;
                    self.rate = NonZeroU32::new(rate);
                }
            }
            "sample_direction" => {
                self.direction = SampleDirection::parse(value)
                    .ok_or_else(|| format!("Invalid sample_direction: {}", value))?;
            }
            _ => {
                // Unknown field - ignore
            }
        }
        Ok(())
    }
}

impl Default for SflowConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_direction_parse() {
        assert_eq!(SampleDirection::parse("rx"), Some(SampleDirection::Rx));
        assert_eq!(SampleDirection::parse("tx"), Some(SampleDirection::Tx));
        assert_eq!(SampleDirection::parse("both"), Some(SampleDirection::Both));
        assert_eq!(SampleDirection::parse("RX"), Some(SampleDirection::Rx));
        assert_eq!(SampleDirection::parse("invalid"), None);
    }

    #[test]
    fn test_sample_direction_has_ingress() {
        assert!(SampleDirection::Rx.has_ingress());
        assert!(!SampleDirection::Tx.has_ingress());
        assert!(SampleDirection::Both.has_ingress());
    }

    #[test]
    fn test_sample_direction_has_egress() {
        assert!(!SampleDirection::Rx.has_egress());
        assert!(SampleDirection::Tx.has_egress());
        assert!(SampleDirection::Both.has_egress());
    }

    #[test]
    fn test_sflow_session() {
        let rate = NonZeroU32::new(4096).unwrap();
        let mut session = SflowSession::new(0x1234, rate);

        assert_eq!(session.ref_count, 0);
        session.add_ref();
        assert_eq!(session.ref_count, 1);
        session.add_ref();
        assert_eq!(session.ref_count, 2);

        assert_eq!(session.remove_ref(), 1);
        assert_eq!(session.remove_ref(), 0);
        assert_eq!(session.remove_ref(), 0); // Saturating sub
    }

    #[test]
    fn test_sflow_config_parse() {
        let mut config = SflowConfig::new();

        config.parse_field("admin_state", "up").unwrap();
        assert!(config.admin_state);

        config.parse_field("sample_rate", "4096").unwrap();
        assert_eq!(config.rate, NonZeroU32::new(4096));

        config.parse_field("sample_direction", "both").unwrap();
        assert_eq!(config.direction, SampleDirection::Both);

        // Error case
        config.parse_field("sample_rate", "error").unwrap();
        assert_eq!(config.rate, None);

        // Invalid rate
        assert!(config.parse_field("sample_rate", "invalid").is_err());
    }

    #[test]
    fn test_sflow_config_zero_rate() {
        let mut config = SflowConfig::new();
        config.parse_field("sample_rate", "0").unwrap();
        assert_eq!(config.rate, None); // NonZeroU32::new(0) returns None
    }
}
