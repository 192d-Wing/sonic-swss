//! VRF types and data structures.

use sonic_sai::types::RawSaiObjectId;
use sonic_types::MacAddress;
use std::fmt;
use std::str::FromStr;

/// VLAN ID type (raw u16 for VRF-internal use).
pub type VrfVlanId = u16;

/// VRF name (string identifier).
pub type VrfName = String;

/// VRF SAI object ID.
pub type VrfId = RawSaiObjectId;

/// VXLAN Network Identifier.
pub type Vni = u32;

/// VRF entry storing the SAI object ID and reference count.
#[derive(Debug, Clone)]
pub struct VrfEntry {
    /// SAI virtual router object ID.
    pub vrf_id: VrfId,
    /// Reference count (number of interfaces using this VRF).
    pub ref_count: i32,
    /// IPv4 admin state (enabled by default).
    pub admin_v4_state: bool,
    /// IPv6 admin state (enabled by default).
    pub admin_v6_state: bool,
    /// Source MAC address (optional override).
    pub src_mac: Option<MacAddress>,
    /// TTL=1 packet action.
    pub ttl_action: Option<PacketAction>,
    /// IP options packet action.
    pub ip_opt_action: Option<PacketAction>,
    /// Unknown L3 multicast packet action.
    pub l3_mc_action: Option<PacketAction>,
    /// Fallback routing enabled.
    pub fallback: bool,
    /// Associated VNI (for EVPN).
    pub vni: Option<Vni>,
}

impl VrfEntry {
    /// Creates a new VRF entry with the given SAI ID.
    pub fn new(vrf_id: VrfId) -> Self {
        Self {
            vrf_id,
            ref_count: 0,
            admin_v4_state: true,
            admin_v6_state: true,
            src_mac: None,
            ttl_action: None,
            ip_opt_action: None,
            l3_mc_action: None,
            fallback: false,
            vni: None,
        }
    }

    /// Increments the reference count.
    pub fn incr_ref_count(&mut self) {
        self.ref_count += 1;
    }

    /// Decrements the reference count.
    /// Returns the new count, or None if it would underflow.
    pub fn decr_ref_count(&mut self) -> Option<i32> {
        if self.ref_count > 0 {
            self.ref_count -= 1;
            Some(self.ref_count)
        } else {
            None
        }
    }

    /// Returns true if this VRF is in use (ref_count > 0).
    pub fn is_in_use(&self) -> bool {
        self.ref_count > 0
    }
}

impl fmt::Display for VrfEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VrfEntry(id=0x{:x}, refs={}, v4={}, v6={})",
            self.vrf_id, self.ref_count, self.admin_v4_state, self.admin_v6_state
        )
    }
}

/// L3 VNI entry for EVPN mapping.
#[derive(Debug, Clone)]
pub struct L3VniEntry {
    /// Associated VLAN ID (0 if not yet mapped).
    pub vlan_id: VrfVlanId,
    /// Whether this is an L3 VNI (vs L2).
    pub l3_vni: bool,
}

impl L3VniEntry {
    /// Creates a new L3 VNI entry.
    pub fn new(vlan_id: VrfVlanId, l3_vni: bool) -> Self {
        Self { vlan_id, l3_vni }
    }

    /// Creates an L3 VNI entry without VLAN mapping yet.
    pub fn pending() -> Self {
        Self {
            vlan_id: 0,
            l3_vni: true,
        }
    }
}

impl Default for L3VniEntry {
    fn default() -> Self {
        Self::pending()
    }
}

/// SAI packet action for VRF violation handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PacketAction {
    /// Drop the packet.
    Drop,
    /// Forward the packet.
    Forward,
    /// Copy packet to CPU and forward.
    Copy,
    /// Copy packet to CPU only (don't forward).
    CopyCancel,
    /// Trap packet to CPU.
    Trap,
    /// Log the packet.
    Log,
    /// Deny (drop with logging).
    Deny,
    /// Transit (forward without modification).
    Transit,
}

impl PacketAction {
    /// Converts to SAI packet action value.
    pub fn to_sai_value(self) -> i32 {
        match self {
            Self::Drop => 0,       // SAI_PACKET_ACTION_DROP
            Self::Forward => 1,    // SAI_PACKET_ACTION_FORWARD
            Self::Copy => 2,       // SAI_PACKET_ACTION_COPY
            Self::CopyCancel => 3, // SAI_PACKET_ACTION_COPY_CANCEL
            Self::Trap => 4,       // SAI_PACKET_ACTION_TRAP
            Self::Log => 5,        // SAI_PACKET_ACTION_LOG
            Self::Deny => 6,       // SAI_PACKET_ACTION_DENY
            Self::Transit => 7,    // SAI_PACKET_ACTION_TRANSIT
        }
    }
}

impl FromStr for PacketAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "DROP" => Ok(Self::Drop),
            "FORWARD" => Ok(Self::Forward),
            "COPY" => Ok(Self::Copy),
            "COPY_CANCEL" => Ok(Self::CopyCancel),
            "TRAP" => Ok(Self::Trap),
            "LOG" => Ok(Self::Log),
            "DENY" => Ok(Self::Deny),
            "TRANSIT" => Ok(Self::Transit),
            _ => Err(format!("Unknown packet action: {}", s)),
        }
    }
}

impl fmt::Display for PacketAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Drop => write!(f, "DROP"),
            Self::Forward => write!(f, "FORWARD"),
            Self::Copy => write!(f, "COPY"),
            Self::CopyCancel => write!(f, "COPY_CANCEL"),
            Self::Trap => write!(f, "TRAP"),
            Self::Log => write!(f, "LOG"),
            Self::Deny => write!(f, "DENY"),
            Self::Transit => write!(f, "TRANSIT"),
        }
    }
}

/// VRF configuration from CONFIG_DB.
#[derive(Debug, Clone, Default)]
pub struct VrfConfig {
    /// VRF name.
    pub name: VrfName,
    /// IPv4 admin state.
    pub v4: Option<bool>,
    /// IPv6 admin state.
    pub v6: Option<bool>,
    /// Source MAC address.
    pub src_mac: Option<MacAddress>,
    /// TTL=1 packet action.
    pub ttl_action: Option<PacketAction>,
    /// IP options packet action.
    pub ip_opt_action: Option<PacketAction>,
    /// Unknown L3 multicast packet action.
    pub l3_mc_action: Option<PacketAction>,
    /// Fallback routing enabled.
    pub fallback: Option<bool>,
    /// Associated VNI.
    pub vni: Option<Vni>,
}

impl VrfConfig {
    /// Creates a new VRF config with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Sets the IPv4 admin state.
    pub fn with_v4(mut self, enabled: bool) -> Self {
        self.v4 = Some(enabled);
        self
    }

    /// Sets the IPv6 admin state.
    pub fn with_v6(mut self, enabled: bool) -> Self {
        self.v6 = Some(enabled);
        self
    }

    /// Sets the source MAC address.
    pub fn with_src_mac(mut self, mac: MacAddress) -> Self {
        self.src_mac = Some(mac);
        self
    }

    /// Sets the TTL action.
    pub fn with_ttl_action(mut self, action: PacketAction) -> Self {
        self.ttl_action = Some(action);
        self
    }

    /// Sets the IP options action.
    pub fn with_ip_opt_action(mut self, action: PacketAction) -> Self {
        self.ip_opt_action = Some(action);
        self
    }

    /// Sets the L3 multicast action.
    pub fn with_l3_mc_action(mut self, action: PacketAction) -> Self {
        self.l3_mc_action = Some(action);
        self
    }

    /// Sets the fallback flag.
    pub fn with_fallback(mut self, enabled: bool) -> Self {
        self.fallback = Some(enabled);
        self
    }

    /// Sets the VNI.
    pub fn with_vni(mut self, vni: Vni) -> Self {
        self.vni = Some(vni);
        self
    }

    /// Parses a field-value pair from CONFIG_DB.
    pub fn parse_field(&mut self, field: &str, value: &str) -> Result<(), String> {
        match field {
            "v4" => {
                self.v4 = Some(parse_bool(value)?);
            }
            "v6" => {
                self.v6 = Some(parse_bool(value)?);
            }
            "src_mac" => {
                self.src_mac = Some(
                    value
                        .parse()
                        .map_err(|_| format!("Invalid MAC address: {}", value))?,
                );
            }
            "ttl_action" => {
                self.ttl_action = Some(value.parse()?);
            }
            "ip_opt_action" => {
                self.ip_opt_action = Some(value.parse()?);
            }
            "l3_mc_action" => {
                self.l3_mc_action = Some(value.parse()?);
            }
            "fallback" => {
                self.fallback = Some(parse_bool(value)?);
            }
            "vni" => {
                self.vni = Some(
                    value
                        .parse()
                        .map_err(|_| format!("Invalid VNI: {}", value))?,
                );
            }
            "mgmtVrfEnabled" | "in_band_mgmt_enabled" => {
                // These fields are ignored per C++ implementation
            }
            _ => {
                return Err(format!("Unknown VRF field: {}", field));
            }
        }
        Ok(())
    }
}

/// Parses a boolean from common string representations.
fn parse_bool(s: &str) -> Result<bool, String> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" | "enabled" => Ok(true),
        "false" | "0" | "no" | "off" | "disabled" => Ok(false),
        _ => Err(format!("Invalid boolean: {}", s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vrf_entry_new() {
        let entry = VrfEntry::new(0x1234);
        assert_eq!(entry.vrf_id, 0x1234);
        assert_eq!(entry.ref_count, 0);
        assert!(entry.admin_v4_state);
        assert!(entry.admin_v6_state);
    }

    #[test]
    fn test_vrf_entry_ref_count() {
        let mut entry = VrfEntry::new(0x1234);
        assert!(!entry.is_in_use());

        entry.incr_ref_count();
        assert!(entry.is_in_use());
        assert_eq!(entry.ref_count, 1);

        entry.incr_ref_count();
        assert_eq!(entry.ref_count, 2);

        assert_eq!(entry.decr_ref_count(), Some(1));
        assert_eq!(entry.decr_ref_count(), Some(0));
        assert_eq!(entry.decr_ref_count(), None); // Underflow protection
    }

    #[test]
    fn test_l3vni_entry() {
        let entry = L3VniEntry::new(100, true);
        assert_eq!(entry.vlan_id, 100);
        assert!(entry.l3_vni);

        let pending = L3VniEntry::pending();
        assert_eq!(pending.vlan_id, 0);
        assert!(pending.l3_vni);
    }

    #[test]
    fn test_packet_action_parse() {
        assert_eq!("DROP".parse::<PacketAction>().unwrap(), PacketAction::Drop);
        assert_eq!(
            "forward".parse::<PacketAction>().unwrap(),
            PacketAction::Forward
        );
        assert_eq!("TRAP".parse::<PacketAction>().unwrap(), PacketAction::Trap);
        assert!("invalid".parse::<PacketAction>().is_err());
    }

    #[test]
    fn test_packet_action_sai_value() {
        assert_eq!(PacketAction::Drop.to_sai_value(), 0);
        assert_eq!(PacketAction::Forward.to_sai_value(), 1);
        assert_eq!(PacketAction::Trap.to_sai_value(), 4);
    }

    #[test]
    fn test_vrf_config() {
        let config = VrfConfig::new("Vrf1")
            .with_v4(true)
            .with_v6(false)
            .with_vni(10000);

        assert_eq!(config.name, "Vrf1");
        assert_eq!(config.v4, Some(true));
        assert_eq!(config.v6, Some(false));
        assert_eq!(config.vni, Some(10000));
    }

    #[test]
    fn test_vrf_config_parse_field() {
        let mut config = VrfConfig::new("Vrf1");

        config.parse_field("v4", "true").unwrap();
        assert_eq!(config.v4, Some(true));

        config.parse_field("vni", "10000").unwrap();
        assert_eq!(config.vni, Some(10000));

        config.parse_field("ttl_action", "DROP").unwrap();
        assert_eq!(config.ttl_action, Some(PacketAction::Drop));

        // Ignored fields should not error
        config.parse_field("mgmtVrfEnabled", "true").unwrap();
    }

    #[test]
    fn test_parse_bool() {
        assert!(parse_bool("true").unwrap());
        assert!(parse_bool("1").unwrap());
        assert!(parse_bool("yes").unwrap());
        assert!(!parse_bool("false").unwrap());
        assert!(!parse_bool("0").unwrap());
        assert!(!parse_bool("no").unwrap());
        assert!(parse_bool("invalid").is_err());
    }
}
