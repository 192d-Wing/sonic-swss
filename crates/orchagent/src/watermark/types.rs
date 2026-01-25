//! Watermark types and data structures.

use sonic_sai::types::RawSaiObjectId;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

/// Default telemetry interval in seconds.
pub const DEFAULT_TELEMETRY_INTERVAL: u64 = 120;

/// Watermark group for flex counter status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatermarkGroup {
    /// Queue watermark.
    Queue,
    /// Priority Group watermark.
    PriorityGroup,
}

impl WatermarkGroup {
    /// Returns the status mask for this group.
    pub fn status_mask(self) -> u8 {
        match self {
            Self::Queue => 0x01,
            Self::PriorityGroup => 0x02,
        }
    }

    /// Returns the flex counter group name.
    pub fn flex_counter_name(&self) -> &'static str {
        match self {
            Self::Queue => "QUEUE_WATERMARK",
            Self::PriorityGroup => "PG_WATERMARK",
        }
    }
}

impl FromStr for WatermarkGroup {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "QUEUE_WATERMARK" => Ok(Self::Queue),
            "PG_WATERMARK" => Ok(Self::PriorityGroup),
            _ => Err(format!("Unknown watermark group: {}", s)),
        }
    }
}

impl fmt::Display for WatermarkGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.flex_counter_name())
    }
}

/// Watermark status tracking which groups are enabled.
#[derive(Debug, Clone, Copy, Default)]
pub struct WatermarkStatus {
    /// Bitmask of enabled groups.
    status: u8,
}

impl WatermarkStatus {
    /// Creates a new status with all groups disabled.
    pub fn new() -> Self {
        Self { status: 0 }
    }

    /// Returns true if any watermark group is enabled.
    pub fn any_enabled(&self) -> bool {
        self.status != 0
    }

    /// Returns true if the specified group is enabled.
    pub fn is_enabled(&self, group: WatermarkGroup) -> bool {
        (self.status & group.status_mask()) != 0
    }

    /// Enables a watermark group.
    pub fn enable(&mut self, group: WatermarkGroup) {
        self.status |= group.status_mask();
    }

    /// Disables a watermark group.
    pub fn disable(&mut self, group: WatermarkGroup) {
        self.status &= !group.status_mask();
    }

    /// Returns true if queue watermarks are enabled.
    pub fn queue_enabled(&self) -> bool {
        self.is_enabled(WatermarkGroup::Queue)
    }

    /// Returns true if PG watermarks are enabled.
    pub fn pg_enabled(&self) -> bool {
        self.is_enabled(WatermarkGroup::PriorityGroup)
    }

    /// Returns the raw status value.
    pub fn raw(&self) -> u8 {
        self.status
    }
}

/// Watermark table type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatermarkTable {
    /// Periodic watermark table (cleared by timer).
    Periodic,
    /// Persistent watermark table (cleared manually).
    Persistent,
    /// User watermark table (cleared by user request).
    User,
}

impl WatermarkTable {
    /// Returns the Redis table name.
    pub fn table_name(&self) -> &'static str {
        match self {
            Self::Periodic => "PERIODIC_WATERMARKS",
            Self::Persistent => "PERSISTENT_WATERMARKS",
            Self::User => "USER_WATERMARKS",
        }
    }
}

impl FromStr for WatermarkTable {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PERIODIC" => Ok(Self::Periodic),
            "PERSISTENT" => Ok(Self::Persistent),
            "USER" => Ok(Self::User),
            _ => Err(format!("Unknown watermark table: {}", s)),
        }
    }
}

impl fmt::Display for WatermarkTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Periodic => write!(f, "PERIODIC"),
            Self::Persistent => write!(f, "PERSISTENT"),
            Self::User => write!(f, "USER"),
        }
    }
}

/// Clear request types for watermark clearing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClearRequest {
    /// Clear PG headroom watermark.
    PgHeadroom,
    /// Clear PG shared watermark.
    PgShared,
    /// Clear queue shared watermark (unicast).
    QueueSharedUnicast,
    /// Clear queue shared watermark (multicast).
    QueueSharedMulticast,
    /// Clear queue shared watermark (all).
    QueueSharedAll,
    /// Clear buffer pool watermark.
    BufferPool,
    /// Clear headroom pool watermark.
    HeadroomPool,
}

impl ClearRequest {
    /// Returns the SAI stat name for this clear request.
    pub fn stat_name(&self) -> &'static str {
        match self {
            Self::PgHeadroom => "SAI_INGRESS_PRIORITY_GROUP_STAT_XOFF_ROOM_WATERMARK_BYTES",
            Self::PgShared => "SAI_INGRESS_PRIORITY_GROUP_STAT_SHARED_WATERMARK_BYTES",
            Self::QueueSharedUnicast | Self::QueueSharedMulticast | Self::QueueSharedAll => {
                "SAI_QUEUE_STAT_SHARED_WATERMARK_BYTES"
            }
            Self::BufferPool => "SAI_BUFFER_POOL_STAT_WATERMARK_BYTES",
            Self::HeadroomPool => "SAI_BUFFER_POOL_STAT_XOFF_ROOM_WATERMARK_BYTES",
        }
    }

    /// Returns the request string.
    pub fn request_name(&self) -> &'static str {
        match self {
            Self::PgHeadroom => "PG_HEADROOM",
            Self::PgShared => "PG_SHARED",
            Self::QueueSharedUnicast => "Q_SHARED_UNI",
            Self::QueueSharedMulticast => "Q_SHARED_MULTI",
            Self::QueueSharedAll => "Q_SHARED_ALL",
            Self::BufferPool => "BUFFER_POOL",
            Self::HeadroomPool => "HEADROOM_POOL",
        }
    }
}

impl FromStr for ClearRequest {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PG_HEADROOM" => Ok(Self::PgHeadroom),
            "PG_SHARED" => Ok(Self::PgShared),
            "Q_SHARED_UNI" => Ok(Self::QueueSharedUnicast),
            "Q_SHARED_MULTI" => Ok(Self::QueueSharedMulticast),
            "Q_SHARED_ALL" => Ok(Self::QueueSharedAll),
            "BUFFER_POOL" => Ok(Self::BufferPool),
            "HEADROOM_POOL" => Ok(Self::HeadroomPool),
            _ => Err(format!("Unknown clear request: {}", s)),
        }
    }
}

impl fmt::Display for ClearRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.request_name())
    }
}

/// Queue type for watermark tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueueType {
    /// Unicast queue.
    Unicast,
    /// Multicast queue.
    Multicast,
    /// All queue types.
    All,
}

impl FromStr for QueueType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SAI_QUEUE_TYPE_UNICAST" => Ok(Self::Unicast),
            "SAI_QUEUE_TYPE_MULTICAST" => Ok(Self::Multicast),
            "SAI_QUEUE_TYPE_ALL" => Ok(Self::All),
            _ => Err(format!("Unknown queue type: {}", s)),
        }
    }
}

/// Watermark configuration from CONFIG_DB.
#[derive(Debug, Clone)]
pub struct WatermarkConfig {
    /// Telemetry interval.
    pub telemetry_interval: Duration,
}

impl Default for WatermarkConfig {
    fn default() -> Self {
        Self {
            telemetry_interval: Duration::from_secs(DEFAULT_TELEMETRY_INTERVAL),
        }
    }
}

impl WatermarkConfig {
    /// Creates a new config with the given telemetry interval.
    pub fn new(telemetry_interval: Duration) -> Self {
        Self { telemetry_interval }
    }

    /// Sets the telemetry interval from seconds.
    pub fn with_interval_secs(mut self, secs: u64) -> Self {
        self.telemetry_interval = Duration::from_secs(secs);
        self
    }
}

/// Queue ID collections for watermark clearing.
#[derive(Debug, Clone, Default)]
pub struct QueueIds {
    /// Unicast queue IDs.
    pub unicast: Vec<RawSaiObjectId>,
    /// Multicast queue IDs.
    pub multicast: Vec<RawSaiObjectId>,
    /// All queue IDs.
    pub all: Vec<RawSaiObjectId>,
}

impl QueueIds {
    /// Creates empty queue ID collections.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if all collections are empty.
    pub fn is_empty(&self) -> bool {
        self.unicast.is_empty() && self.multicast.is_empty() && self.all.is_empty()
    }

    /// Adds a queue ID based on its type.
    pub fn add(&mut self, queue_type: QueueType, id: RawSaiObjectId) {
        match queue_type {
            QueueType::Unicast => self.unicast.push(id),
            QueueType::Multicast => self.multicast.push(id),
            QueueType::All => self.all.push(id),
        }
    }

    /// Returns queue IDs for a clear request.
    pub fn get_for_clear(&self, request: ClearRequest) -> &[RawSaiObjectId] {
        match request {
            ClearRequest::QueueSharedUnicast => &self.unicast,
            ClearRequest::QueueSharedMulticast => &self.multicast,
            ClearRequest::QueueSharedAll => &self.all,
            _ => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watermark_group() {
        assert_eq!(WatermarkGroup::Queue.status_mask(), 0x01);
        assert_eq!(WatermarkGroup::PriorityGroup.status_mask(), 0x02);

        assert_eq!(
            "QUEUE_WATERMARK".parse::<WatermarkGroup>().unwrap(),
            WatermarkGroup::Queue
        );
        assert_eq!(
            "PG_WATERMARK".parse::<WatermarkGroup>().unwrap(),
            WatermarkGroup::PriorityGroup
        );
    }

    #[test]
    fn test_watermark_status() {
        let mut status = WatermarkStatus::new();
        assert!(!status.any_enabled());

        status.enable(WatermarkGroup::Queue);
        assert!(status.any_enabled());
        assert!(status.queue_enabled());
        assert!(!status.pg_enabled());

        status.enable(WatermarkGroup::PriorityGroup);
        assert!(status.pg_enabled());

        status.disable(WatermarkGroup::Queue);
        assert!(!status.queue_enabled());
        assert!(status.pg_enabled());
        assert!(status.any_enabled());

        status.disable(WatermarkGroup::PriorityGroup);
        assert!(!status.any_enabled());
    }

    #[test]
    fn test_watermark_table() {
        assert_eq!(
            "PERIODIC".parse::<WatermarkTable>().unwrap(),
            WatermarkTable::Periodic
        );
        assert_eq!(
            "PERSISTENT".parse::<WatermarkTable>().unwrap(),
            WatermarkTable::Persistent
        );
        assert_eq!(
            "USER".parse::<WatermarkTable>().unwrap(),
            WatermarkTable::User
        );
    }

    #[test]
    fn test_clear_request() {
        assert_eq!(
            "PG_HEADROOM".parse::<ClearRequest>().unwrap(),
            ClearRequest::PgHeadroom
        );
        assert_eq!(
            "Q_SHARED_UNI".parse::<ClearRequest>().unwrap(),
            ClearRequest::QueueSharedUnicast
        );
        assert_eq!(
            "BUFFER_POOL".parse::<ClearRequest>().unwrap(),
            ClearRequest::BufferPool
        );

        assert_eq!(
            ClearRequest::PgHeadroom.stat_name(),
            "SAI_INGRESS_PRIORITY_GROUP_STAT_XOFF_ROOM_WATERMARK_BYTES"
        );
    }

    #[test]
    fn test_queue_ids() {
        let mut ids = QueueIds::new();
        assert!(ids.is_empty());

        ids.add(QueueType::Unicast, 1);
        ids.add(QueueType::Multicast, 2);
        ids.add(QueueType::All, 3);

        assert!(!ids.is_empty());
        assert_eq!(ids.unicast.len(), 1);
        assert_eq!(ids.multicast.len(), 1);
        assert_eq!(ids.all.len(), 1);

        assert_eq!(ids.get_for_clear(ClearRequest::QueueSharedUnicast), &[1]);
        assert_eq!(ids.get_for_clear(ClearRequest::QueueSharedMulticast), &[2]);
    }

    #[test]
    fn test_watermark_config() {
        let config = WatermarkConfig::default();
        assert_eq!(config.telemetry_interval, Duration::from_secs(120));

        let config = WatermarkConfig::default().with_interval_secs(60);
        assert_eq!(config.telemetry_interval, Duration::from_secs(60));
    }
}
