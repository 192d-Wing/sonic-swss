//! PolicerOrch types.

use sonic_sai::types::RawSaiObjectId;

/// Policer meter type (what to measure).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MeterType {
    /// Meter based on packet count.
    Packets,
    /// Meter based on byte count.
    Bytes,
}

impl MeterType {
    /// Parses a meter type string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "PACKETS" => Some(Self::Packets),
            "BYTES" => Some(Self::Bytes),
            _ => None,
        }
    }

    /// Returns the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Packets => "PACKETS",
            Self::Bytes => "BYTES",
        }
    }
}

/// Policer mode (algorithm).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolicerMode {
    /// Single Rate Three Color Marker.
    SrTcm,
    /// Two Rate Three Color Marker.
    TrTcm,
    /// Storm control mode.
    StormControl,
}

impl PolicerMode {
    /// Parses a policer mode string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "SR_TCM" => Some(Self::SrTcm),
            "TR_TCM" => Some(Self::TrTcm),
            "STORM_CONTROL" => Some(Self::StormControl),
            _ => None,
        }
    }

    /// Returns the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SrTcm => "SR_TCM",
            Self::TrTcm => "TR_TCM",
            Self::StormControl => "STORM_CONTROL",
        }
    }
}

/// Color source (color awareness).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorSource {
    /// Color-aware (considers incoming packet color).
    Aware,
    /// Color-blind (ignores incoming packet color).
    Blind,
}

impl ColorSource {
    /// Parses a color source string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "AWARE" => Some(Self::Aware),
            "BLIND" => Some(Self::Blind),
            _ => None,
        }
    }

    /// Returns the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Aware => "AWARE",
            Self::Blind => "BLIND",
        }
    }
}

/// Packet action for colored packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PacketAction {
    /// Drop the packet.
    Drop,
    /// Forward the packet normally.
    Forward,
    /// Copy to CPU.
    Copy,
    /// Cancel copy to CPU.
    CopyCancel,
    /// Trap to CPU.
    Trap,
    /// Log the packet.
    Log,
    /// Deny forwarding.
    Deny,
    /// Transit through.
    Transit,
}

impl PacketAction {
    /// Parses a packet action string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "DROP" => Some(Self::Drop),
            "FORWARD" => Some(Self::Forward),
            "COPY" => Some(Self::Copy),
            "COPY_CANCEL" => Some(Self::CopyCancel),
            "TRAP" => Some(Self::Trap),
            "LOG" => Some(Self::Log),
            "DENY" => Some(Self::Deny),
            "TRANSIT" => Some(Self::Transit),
            _ => None,
        }
    }

    /// Returns the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Drop => "DROP",
            Self::Forward => "FORWARD",
            Self::Copy => "COPY",
            Self::CopyCancel => "COPY_CANCEL",
            Self::Trap => "TRAP",
            Self::Log => "LOG",
            Self::Deny => "DENY",
            Self::Transit => "TRANSIT",
        }
    }
}

/// Storm control type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StormType {
    /// Broadcast storm control.
    Broadcast,
    /// Unknown unicast (flood) storm control.
    UnknownUnicast,
    /// Unknown multicast storm control.
    UnknownMulticast,
}

impl StormType {
    /// Parses a storm type string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "broadcast" => Some(Self::Broadcast),
            "unknown-unicast" => Some(Self::UnknownUnicast),
            "unknown-multicast" => Some(Self::UnknownMulticast),
            _ => None,
        }
    }

    /// Returns the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Broadcast => "broadcast",
            Self::UnknownUnicast => "unknown-unicast",
            Self::UnknownMulticast => "unknown-multicast",
        }
    }
}

/// Policer configuration.
#[derive(Debug, Clone)]
pub struct PolicerConfig {
    /// Meter type (packets or bytes).
    pub meter_type: MeterType,
    /// Policer mode (SR_TCM, TR_TCM, STORM_CONTROL).
    pub mode: PolicerMode,
    /// Color source (aware or blind).
    pub color_source: ColorSource,
    /// Committed information rate (bytes/packets per second).
    pub cir: u64,
    /// Committed burst size (bytes/packets).
    pub cbs: u64,
    /// Peak information rate (bytes/packets per second).
    pub pir: u64,
    /// Peak burst size (bytes/packets).
    pub pbs: u64,
    /// Action for green packets.
    pub green_action: PacketAction,
    /// Action for yellow packets.
    pub yellow_action: PacketAction,
    /// Action for red packets.
    pub red_action: PacketAction,
}

impl PolicerConfig {
    /// Creates a new policer config with defaults.
    pub fn new() -> Self {
        Self {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::SrTcm,
            color_source: ColorSource::Blind,
            cir: 0,
            cbs: 0,
            pir: 0,
            pbs: 0,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        }
    }

    /// Creates a storm control policer config.
    pub fn storm_control(kbps: u64) -> Self {
        // Convert kbps to bytes per second: (kbps * 1000 / 8)
        let cir_bps = kbps.saturating_mul(1000).saturating_div(8);

        Self {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::StormControl,
            color_source: ColorSource::Blind,
            cir: cir_bps,
            cbs: 0, // Use hardware defaults
            pir: 0,
            pbs: 0,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        }
    }

    /// Parses a field-value pair and updates the config.
    pub fn parse_field(&mut self, field: &str, value: &str) -> Result<(), String> {
        match field {
            "meter_type" => {
                self.meter_type = MeterType::parse(value)
                    .ok_or_else(|| format!("Invalid meter_type: {}", value))?;
            }
            "mode" => {
                self.mode = PolicerMode::parse(value)
                    .ok_or_else(|| format!("Invalid mode: {}", value))?;
            }
            "color_source" => {
                self.color_source = ColorSource::parse(value)
                    .ok_or_else(|| format!("Invalid color_source: {}", value))?;
            }
            "cir" => {
                self.cir = value
                    .parse::<u64>()
                    .map_err(|e| format!("Invalid cir '{}': {}", value, e))?;
            }
            "cbs" => {
                self.cbs = value
                    .parse::<u64>()
                    .map_err(|e| format!("Invalid cbs '{}': {}", value, e))?;
            }
            "pir" => {
                self.pir = value
                    .parse::<u64>()
                    .map_err(|e| format!("Invalid pir '{}': {}", value, e))?;
            }
            "pbs" => {
                self.pbs = value
                    .parse::<u64>()
                    .map_err(|e| format!("Invalid pbs '{}': {}", value, e))?;
            }
            "green_packet_action" => {
                self.green_action = PacketAction::parse(value)
                    .ok_or_else(|| format!("Invalid green_packet_action: {}", value))?;
            }
            "yellow_packet_action" => {
                self.yellow_action = PacketAction::parse(value)
                    .ok_or_else(|| format!("Invalid yellow_packet_action: {}", value))?;
            }
            "red_packet_action" => {
                self.red_action = PacketAction::parse(value)
                    .ok_or_else(|| format!("Invalid red_packet_action: {}", value))?;
            }
            _ => {
                // Unknown field - ignore
            }
        }
        Ok(())
    }

    /// Returns true if only rate/burst parameters changed (updatable).
    pub fn is_rate_burst_update(&self, other: &Self) -> bool {
        self.meter_type == other.meter_type
            && self.mode == other.mode
            && self.color_source == other.color_source
            && self.green_action == other.green_action
            && self.yellow_action == other.yellow_action
            && self.red_action == other.red_action
    }
}

impl Default for PolicerConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Policer entry.
#[derive(Debug, Clone)]
pub struct PolicerEntry {
    /// SAI policer object ID.
    pub sai_oid: RawSaiObjectId,
    /// Policer configuration.
    pub config: PolicerConfig,
    /// Reference count (number of users).
    pub ref_count: u32,
}

impl PolicerEntry {
    /// Creates a new policer entry.
    pub fn new(sai_oid: RawSaiObjectId, config: PolicerConfig) -> Self {
        Self {
            sai_oid,
            config,
            ref_count: 0,
        }
    }

    /// Increments the reference count.
    pub fn add_ref(&mut self) {
        self.ref_count = self.ref_count.saturating_add(1);
    }

    /// Decrements the reference count.
    pub fn remove_ref(&mut self) -> u32 {
        self.ref_count = self.ref_count.saturating_sub(1);
        self.ref_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meter_type_parse() {
        assert_eq!(MeterType::parse("PACKETS"), Some(MeterType::Packets));
        assert_eq!(MeterType::parse("BYTES"), Some(MeterType::Bytes));
        assert_eq!(MeterType::parse("packets"), Some(MeterType::Packets));
        assert_eq!(MeterType::parse("invalid"), None);
    }

    #[test]
    fn test_policer_mode_parse() {
        assert_eq!(PolicerMode::parse("SR_TCM"), Some(PolicerMode::SrTcm));
        assert_eq!(PolicerMode::parse("TR_TCM"), Some(PolicerMode::TrTcm));
        assert_eq!(
            PolicerMode::parse("STORM_CONTROL"),
            Some(PolicerMode::StormControl)
        );
        assert_eq!(PolicerMode::parse("invalid"), None);
    }

    #[test]
    fn test_color_source_parse() {
        assert_eq!(ColorSource::parse("AWARE"), Some(ColorSource::Aware));
        assert_eq!(ColorSource::parse("BLIND"), Some(ColorSource::Blind));
        assert_eq!(ColorSource::parse("invalid"), None);
    }

    #[test]
    fn test_packet_action_parse() {
        assert_eq!(PacketAction::parse("DROP"), Some(PacketAction::Drop));
        assert_eq!(PacketAction::parse("FORWARD"), Some(PacketAction::Forward));
        assert_eq!(PacketAction::parse("invalid"), None);
    }

    #[test]
    fn test_storm_type_parse() {
        assert_eq!(StormType::parse("broadcast"), Some(StormType::Broadcast));
        assert_eq!(
            StormType::parse("unknown-unicast"),
            Some(StormType::UnknownUnicast)
        );
        assert_eq!(
            StormType::parse("unknown-multicast"),
            Some(StormType::UnknownMulticast)
        );
        assert_eq!(StormType::parse("invalid"), None);
    }

    #[test]
    fn test_policer_config_storm_control() {
        let config = PolicerConfig::storm_control(8000); // 8000 kbps = 1000000 bps = 1MB/s
        assert_eq!(config.meter_type, MeterType::Bytes);
        assert_eq!(config.mode, PolicerMode::StormControl);
        assert_eq!(config.cir, 1000000); // 8000 * 1000 / 8
        assert_eq!(config.red_action, PacketAction::Drop);
    }

    #[test]
    fn test_policer_config_parse_field() {
        let mut config = PolicerConfig::new();

        config.parse_field("meter_type", "PACKETS").unwrap();
        assert_eq!(config.meter_type, MeterType::Packets);

        config.parse_field("mode", "TR_TCM").unwrap();
        assert_eq!(config.mode, PolicerMode::TrTcm);

        config.parse_field("cir", "1000000").unwrap();
        assert_eq!(config.cir, 1000000);

        // Invalid value
        assert!(config.parse_field("cir", "invalid").is_err());
    }

    #[test]
    fn test_policer_entry_ref_count() {
        let mut entry = PolicerEntry::new(0x1234, PolicerConfig::new());

        assert_eq!(entry.ref_count, 0);
        entry.add_ref();
        assert_eq!(entry.ref_count, 1);
        entry.add_ref();
        assert_eq!(entry.ref_count, 2);

        assert_eq!(entry.remove_ref(), 1);
        assert_eq!(entry.remove_ref(), 0);
        assert_eq!(entry.remove_ref(), 0); // Saturating sub
    }

    #[test]
    fn test_is_rate_burst_update() {
        let config1 = PolicerConfig {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::SrTcm,
            color_source: ColorSource::Blind,
            cir: 1000,
            cbs: 500,
            pir: 2000,
            pbs: 1000,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        };

        let mut config2 = config1.clone();
        config2.cir = 2000; // Changed rate

        assert!(config1.is_rate_burst_update(&config2));

        config2.meter_type = MeterType::Packets; // Changed meter type
        assert!(!config1.is_rate_burst_update(&config2));
    }
}
