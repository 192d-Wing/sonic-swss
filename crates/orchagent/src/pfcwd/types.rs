//! PFC Watchdog types and structures.

use sonic_sai::types::RawSaiObjectId;

/// PFC watchdog action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PfcWdAction {
    Unknown,
    Forward,
    Drop,
    Alert,
}

impl PfcWdAction {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "forward" => Some(Self::Forward),
            "drop" => Some(Self::Drop),
            "alert" => Some(Self::Alert),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Forward => "forward",
            Self::Drop => "drop",
            Self::Alert => "alert",
        }
    }
}

/// Detection time (100-5000 ms).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectionTime(u32);

impl DetectionTime {
    pub fn new(value: u32) -> Result<Self, String> {
        if value >= 100 && value <= 5000 {
            Ok(Self(value))
        } else {
            Err(format!("Detection time {} must be 100-5000ms", value))
        }
    }

    pub fn value(&self) -> u32 {
        self.0
    }
}

/// Restoration time (0-60000 ms, 0 = disabled).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestorationTime(u32);

impl RestorationTime {
    pub fn new(value: u32) -> Result<Self, String> {
        if value <= 60000 {
            Ok(Self(value))
        } else {
            Err(format!("Restoration time {} exceeds 60000ms", value))
        }
    }

    pub fn value(&self) -> u32 {
        self.0
    }
}

/// PFC watchdog configuration.
#[derive(Debug, Clone)]
pub struct PfcWdConfig {
    pub queue_name: String,
    pub detection_time: DetectionTime,
    pub restoration_time: RestorationTime,
    pub action: PfcWdAction,
}

impl PfcWdConfig {
    pub fn new(
        queue_name: String,
        action: PfcWdAction,
        detection_time: DetectionTime,
        restoration_time: RestorationTime,
    ) -> Self {
        Self {
            queue_name,
            detection_time,
            restoration_time,
            action,
        }
    }
}

/// PFC watchdog queue entry.
#[derive(Debug, Clone)]
pub struct PfcWdQueueEntry {
    pub action: PfcWdAction,
    pub port_id: RawSaiObjectId,
    pub queue_index: u8,
    pub port_alias: String,
}

/// PFC watchdog entry.
#[derive(Debug, Clone)]
pub struct PfcWdEntry {
    pub queue_name: String,
    pub watchdog_id: RawSaiObjectId,
    pub action: PfcWdAction,
    pub detection_time: DetectionTime,
    pub restoration_time: RestorationTime,
    pub enabled: bool,
    pub storm_detected: bool,
}

impl PfcWdEntry {
    pub fn from_config(config: PfcWdConfig, watchdog_id: RawSaiObjectId) -> Self {
        Self {
            queue_name: config.queue_name,
            watchdog_id,
            action: config.action,
            detection_time: config.detection_time,
            restoration_time: config.restoration_time,
            enabled: false,
            storm_detected: false,
        }
    }
}

/// PFC watchdog statistics.
#[derive(Debug, Clone, Default)]
pub struct PfcWdStats {
    pub storms_detected: u64,
    pub storms_restored: u64,
}

/// Hardware statistics snapshot.
#[derive(Debug, Clone, Default)]
pub struct PfcWdHwStats {
    pub tx_pkt: u64,
    pub tx_drop_pkt: u64,
    pub rx_pkt: u64,
    pub rx_drop_pkt: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_parse() {
        assert_eq!(PfcWdAction::parse("forward"), Some(PfcWdAction::Forward));
        assert_eq!(PfcWdAction::parse("DROP"), Some(PfcWdAction::Drop));
    }

    #[test]
    fn test_detection_time() {
        assert!(DetectionTime::new(99).is_err());
        assert!(DetectionTime::new(100).is_ok());
        assert!(DetectionTime::new(5000).is_ok());
        assert!(DetectionTime::new(5001).is_err());
    }
}
