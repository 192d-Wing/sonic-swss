//! Port struct and related types.
//!
//! The Port struct contains all the information for a physical or logical port,
//! including SAI object IDs, configuration, and operational state.

use sonic_sai::types::RawSaiObjectId;
use sonic_types::MacAddress;
use std::collections::{HashMap, HashSet};
use std::fmt;

/// Port type enumeration.
///
/// This replaces the C++ `Port::Type` enum with type-safe Rust variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PortType {
    /// Physical port (front-panel port).
    #[default]
    Phy,
    /// CPU port (for control plane traffic).
    Cpu,
    /// VLAN port (SVI - Switch Virtual Interface).
    Vlan,
    /// LAG (Link Aggregation Group) port.
    Lag,
    /// LAG member port (physical port in a LAG).
    LagMember,
    /// Tunnel port (for overlay networks).
    Tunnel,
    /// Loopback port.
    Loopback,
    /// Subport (VLAN subinterface).
    Subport,
    /// System port (for VOQ/distributed systems).
    System,
    /// Unknown port type.
    Unknown,
}

impl fmt::Display for PortType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Phy => write!(f, "PHY"),
            Self::Cpu => write!(f, "CPU"),
            Self::Vlan => write!(f, "VLAN"),
            Self::Lag => write!(f, "LAG"),
            Self::LagMember => write!(f, "LAG_MEMBER"),
            Self::Tunnel => write!(f, "TUNNEL"),
            Self::Loopback => write!(f, "LOOPBACK"),
            Self::Subport => write!(f, "SUBPORT"),
            Self::System => write!(f, "SYSTEM"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

impl std::str::FromStr for PortType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PHY" => Ok(Self::Phy),
            "CPU" => Ok(Self::Cpu),
            "VLAN" => Ok(Self::Vlan),
            "LAG" => Ok(Self::Lag),
            "LAG_MEMBER" => Ok(Self::LagMember),
            "TUNNEL" => Ok(Self::Tunnel),
            "LOOPBACK" => Ok(Self::Loopback),
            "SUBPORT" => Ok(Self::Subport),
            "SYSTEM" => Ok(Self::System),
            _ => Err(format!("Unknown port type: {}", s)),
        }
    }
}

/// Port role enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortRole {
    /// External port (facing outside the switch).
    #[default]
    Ext,
    /// Internal port (inter-chip connection).
    Int,
    /// Internal non-router port.
    Inb,
    /// Recycle port.
    Rec,
}

impl fmt::Display for PortRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ext => write!(f, "Ext"),
            Self::Int => write!(f, "Int"),
            Self::Inb => write!(f, "Inb"),
            Self::Rec => write!(f, "Rec"),
        }
    }
}

impl std::str::FromStr for PortRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ext" | "external" => Ok(Self::Ext),
            "int" | "internal" => Ok(Self::Int),
            "inb" => Ok(Self::Inb),
            "rec" | "recycle" => Ok(Self::Rec),
            _ => Err(format!("Unknown port role: {}", s)),
        }
    }
}

/// FEC (Forward Error Correction) mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortFecMode {
    /// No FEC.
    #[default]
    None,
    /// Reed-Solomon FEC (RS).
    Rs,
    /// FireCode FEC (FC/BaseR).
    Fc,
    /// Auto-negotiate FEC.
    Auto,
}

impl fmt::Display for PortFecMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Rs => write!(f, "rs"),
            Self::Fc => write!(f, "fc"),
            Self::Auto => write!(f, "auto"),
        }
    }
}

impl std::str::FromStr for PortFecMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" | "off" => Ok(Self::None),
            "rs" | "reed-solomon" => Ok(Self::Rs),
            "fc" | "baser" | "firecode" => Ok(Self::Fc),
            "auto" => Ok(Self::Auto),
            _ => Err(format!("Unknown FEC mode: {}", s)),
        }
    }
}

/// Port admin state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortAdminState {
    /// Admin up (enabled).
    Up,
    /// Admin down (disabled).
    #[default]
    Down,
}

impl fmt::Display for PortAdminState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Up => write!(f, "up"),
            Self::Down => write!(f, "down"),
        }
    }
}

impl std::str::FromStr for PortAdminState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "up" | "enable" | "enabled" | "true" => Ok(Self::Up),
            "down" | "disable" | "disabled" | "false" => Ok(Self::Down),
            _ => Err(format!("Unknown admin state: {}", s)),
        }
    }
}

impl From<bool> for PortAdminState {
    fn from(v: bool) -> Self {
        if v { Self::Up } else { Self::Down }
    }
}

impl From<PortAdminState> for bool {
    fn from(v: PortAdminState) -> Self {
        matches!(v, PortAdminState::Up)
    }
}

/// Port operational state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortOperState {
    /// Operationally up (link established).
    Up,
    /// Operationally down (no link).
    #[default]
    Down,
    /// Unknown state.
    Unknown,
}

impl fmt::Display for PortOperState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Up => write!(f, "up"),
            Self::Down => write!(f, "down"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Port auto-negotiation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortAutoNegMode {
    /// Auto-negotiation enabled.
    #[default]
    Enabled,
    /// Auto-negotiation disabled.
    Disabled,
}

impl fmt::Display for PortAutoNegMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Enabled => write!(f, "enabled"),
            Self::Disabled => write!(f, "disabled"),
        }
    }
}

impl std::str::FromStr for PortAutoNegMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "enabled" | "on" | "true" | "1" => Ok(Self::Enabled),
            "disabled" | "off" | "false" | "0" => Ok(Self::Disabled),
            _ => Err(format!("Unknown auto-neg mode: {}", s)),
        }
    }
}

/// Port interface type (media type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortInterfaceType {
    /// None/unknown.
    #[default]
    None,
    /// CR (Copper/Direct Attach).
    Cr,
    /// CR2.
    Cr2,
    /// CR4.
    Cr4,
    /// CR8.
    Cr8,
    /// SR (Short Range optical).
    Sr,
    /// SR2.
    Sr2,
    /// SR4.
    Sr4,
    /// SR8.
    Sr8,
    /// LR (Long Range optical).
    Lr,
    /// LR4.
    Lr4,
    /// KR (Backplane).
    Kr,
    /// KR2.
    Kr2,
    /// KR4.
    Kr4,
    /// KR8.
    Kr8,
}

impl fmt::Display for PortInterfaceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Cr => write!(f, "CR"),
            Self::Cr2 => write!(f, "CR2"),
            Self::Cr4 => write!(f, "CR4"),
            Self::Cr8 => write!(f, "CR8"),
            Self::Sr => write!(f, "SR"),
            Self::Sr2 => write!(f, "SR2"),
            Self::Sr4 => write!(f, "SR4"),
            Self::Sr8 => write!(f, "SR8"),
            Self::Lr => write!(f, "LR"),
            Self::Lr4 => write!(f, "LR4"),
            Self::Kr => write!(f, "KR"),
            Self::Kr2 => write!(f, "KR2"),
            Self::Kr4 => write!(f, "KR4"),
            Self::Kr8 => write!(f, "KR8"),
        }
    }
}

impl std::str::FromStr for PortInterfaceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "NONE" | "" => Ok(Self::None),
            "CR" => Ok(Self::Cr),
            "CR2" => Ok(Self::Cr2),
            "CR4" => Ok(Self::Cr4),
            "CR8" => Ok(Self::Cr8),
            "SR" => Ok(Self::Sr),
            "SR2" => Ok(Self::Sr2),
            "SR4" => Ok(Self::Sr4),
            "SR8" => Ok(Self::Sr8),
            "LR" => Ok(Self::Lr),
            "LR4" => Ok(Self::Lr4),
            "KR" => Ok(Self::Kr),
            "KR2" => Ok(Self::Kr2),
            "KR4" => Ok(Self::Kr4),
            "KR8" => Ok(Self::Kr8),
            _ => Err(format!("Unknown interface type: {}", s)),
        }
    }
}

/// Link training mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortLinkTrainingMode {
    /// Link training enabled.
    On,
    /// Link training disabled.
    #[default]
    Off,
}

impl fmt::Display for PortLinkTrainingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::On => write!(f, "on"),
            Self::Off => write!(f, "off"),
        }
    }
}

impl std::str::FromStr for PortLinkTrainingMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "on" | "enabled" | "true" | "1" => Ok(Self::On),
            "off" | "disabled" | "false" | "0" => Ok(Self::Off),
            _ => Err(format!("Unknown link training mode: {}", s)),
        }
    }
}

/// Preemphasis setting for SerDes.
#[derive(Debug, Clone, Default)]
pub struct SerdesPreemphasis {
    /// Pre-cursor tap.
    pub pre: i32,
    /// Main cursor tap.
    pub main: i32,
    /// Post-cursor tap.
    pub post: i32,
    /// Post2 cursor tap.
    pub post2: i32,
}

/// Queue statistics structure.
#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    /// Packets transmitted.
    pub tx_packets: u64,
    /// Packets dropped.
    pub tx_drops: u64,
    /// Bytes transmitted.
    pub tx_bytes: u64,
}

/// Priority Group statistics structure.
#[derive(Debug, Clone, Default)]
pub struct PriorityGroupStats {
    /// RX packets.
    pub rx_packets: u64,
    /// RX bytes.
    pub rx_bytes: u64,
    /// Dropped packets.
    pub dropped_packets: u64,
}

/// Port struct - the main data structure representing a port.
///
/// This is the Rust equivalent of the C++ `Port` class, but with:
/// - Owned data instead of raw pointers
/// - Type-safe enums instead of integer constants
/// - Result-based error handling instead of exceptions
///
/// # Safety Improvements
///
/// - No raw pointers that can be null
/// - No auto-vivification (maps return Option)
/// - All fields have safe defaults
/// - Immutable by default, explicit mut required
#[derive(Debug, Clone)]
pub struct Port {
    // ============ Identifiers ============
    /// Port alias (e.g., "Ethernet0", "PortChannel0001", "Vlan100").
    pub alias: String,
    /// Port description from config.
    pub description: String,
    /// Port index (sequential, used for port ID generation).
    pub index: u32,
    /// Port type.
    pub port_type: PortType,
    /// Port role.
    pub role: PortRole,

    // ============ SAI Object IDs ============
    /// SAI port object ID.
    pub port_id: RawSaiObjectId,
    /// SAI bridge port ID (for L2 forwarding).
    pub bridge_port_id: RawSaiObjectId,
    /// SAI VLAN member ID (for untagged VLAN).
    pub vlan_member_id: RawSaiObjectId,
    /// SAI host interface ID (for CPU traffic).
    pub host_if_id: RawSaiObjectId,
    /// SAI router interface ID (for L3 forwarding).
    pub rif_id: RawSaiObjectId,
    /// SAI ingress ACL table ID.
    pub ingress_acl_table_id: RawSaiObjectId,
    /// SAI egress ACL table ID.
    pub egress_acl_table_id: RawSaiObjectId,
    /// SAI ingress mirror session ID.
    pub ingress_mirror_session_id: Option<RawSaiObjectId>,
    /// SAI egress mirror session ID.
    pub egress_mirror_session_id: Option<RawSaiObjectId>,
    /// SAI ingress sample session ID (sFlow).
    pub ingress_sample_session_id: Option<RawSaiObjectId>,
    /// SAI egress sample session ID (sFlow).
    pub egress_sample_session_id: Option<RawSaiObjectId>,

    // ============ Physical Properties ============
    /// Physical lanes used by this port.
    pub lanes: Vec<u32>,
    /// Port speed in Mbps.
    pub speed: u32,
    /// Advertised speeds for auto-negotiation (Mbps).
    pub adv_speeds: Vec<u32>,
    /// Interface type (media type).
    pub interface_type: PortInterfaceType,
    /// Advertised interface types for auto-negotiation.
    pub adv_interface_types: Vec<PortInterfaceType>,
    /// FEC mode.
    pub fec_mode: PortFecMode,
    /// Auto-negotiation mode.
    pub autoneg: PortAutoNegMode,
    /// Link training mode.
    pub link_training: PortLinkTrainingMode,
    /// MTU (Maximum Transmission Unit).
    pub mtu: u32,
    /// TPID (Tag Protocol Identifier) for VLAN tagging.
    pub tpid: u16,
    /// Port preemphasis/SerDes settings.
    pub preemphasis: Option<SerdesPreemphasis>,
    /// SerDes TX/RX lane configuration (lane index → values).
    pub serdes_lane_config: HashMap<u32, Vec<i32>>,

    // ============ Administrative State ============
    /// Admin state (up/down).
    pub admin_state: PortAdminState,
    /// Operational state (up/down).
    pub oper_state: PortOperState,
    /// Whether port is initialized.
    pub initialized: bool,

    // ============ L2 Properties ============
    /// Untagged VLAN (native VLAN) ID.
    pub untagged_vlan: u16,
    /// Port VLAN ID (PVID).
    pub pvid: u16,
    /// MAC address (if assigned, e.g., for SVIs).
    pub mac_address: Option<MacAddress>,
    /// Bridge port admin state.
    pub bridge_port_admin_state: bool,
    /// Whether learn mode is set.
    pub learn_mode_set: bool,

    // ============ LAG Properties ============
    /// Parent LAG ID (if this port is a LAG member).
    pub lag_id: Option<RawSaiObjectId>,
    /// LAG member ID (if this port is a LAG member).
    pub lag_member_id: Option<RawSaiObjectId>,
    /// LAG member ports (if this is a LAG).
    pub lag_members: HashSet<String>,

    // ============ VLAN Properties ============
    /// VLAN ID (if this is a VLAN port/SVI).
    pub vlan_id: u16,
    /// VLAN OID (if this is a VLAN port).
    pub vlan_oid: Option<RawSaiObjectId>,
    /// VLANs this port is a member of.
    pub vlan_members: HashSet<u16>,

    // ============ QoS Properties ============
    /// Queue IDs for this port.
    pub queue_ids: Vec<RawSaiObjectId>,
    /// Number of unicast queues.
    pub num_unicast_queues: u32,
    /// Number of multicast queues.
    pub num_multicast_queues: u32,
    /// Number of all queues.
    pub num_queues: u32,
    /// Priority group IDs.
    pub priority_group_ids: Vec<RawSaiObjectId>,
    /// Number of priority groups.
    pub num_priority_groups: u32,
    /// Scheduler group IDs.
    pub scheduler_group_ids: Vec<RawSaiObjectId>,
    /// Ingress priority group index.
    pub ingress_priority_group_index: HashMap<u32, RawSaiObjectId>,
    /// Queue index mapping (priority → queue).
    pub queue_index_mapping: HashMap<u32, RawSaiObjectId>,

    // ============ ACL Properties ============
    /// Port bind mode for ACL.
    pub acl_port_bind_mode: u32,
    /// Ingress ACL group ID.
    pub ingress_acl_group_id: Option<RawSaiObjectId>,
    /// Egress ACL group ID.
    pub egress_acl_group_id: Option<RawSaiObjectId>,

    // ============ PFC/Flow Control ============
    /// PFC (Priority Flow Control) asymmetric mode.
    pub pfc_asym: bool,
    /// PFC priorities enabled (bitmap).
    pub pfc_priorities: u8,
    /// Global flow control TX enabled.
    pub fc_tx_enable: bool,
    /// Global flow control RX enabled.
    pub fc_rx_enable: bool,

    // ============ Tunnel Properties ============
    /// Tunnel ID (if this is a tunnel port).
    pub tunnel_id: Option<RawSaiObjectId>,
    /// Tunnel type (if applicable).
    pub tunnel_type: Option<String>,

    // ============ System Port Properties (VOQ) ============
    /// System port ID (for distributed systems).
    pub system_port_id: Option<RawSaiObjectId>,
    /// System port number.
    pub system_port_number: Option<u32>,
    /// Core index for VOQ.
    pub core_index: Option<u32>,
    /// Core port index.
    pub core_port_index: Option<u32>,

    // ============ Gearbox Properties ============
    /// Gearbox PHY OID.
    pub gearbox_phy_id: Option<RawSaiObjectId>,
    /// Gearbox port OID.
    pub gearbox_port_id: Option<RawSaiObjectId>,
    /// Line side lanes (for gearbox).
    pub line_lanes: Vec<u32>,
    /// System side lanes (for gearbox).
    pub system_lanes: Vec<u32>,

    // ============ Counters/Statistics ============
    /// Whether debug counters are enabled.
    pub debug_counter_enabled: bool,
    /// Port counter mode.
    pub counter_mode: u32,

    // ============ Miscellaneous ============
    /// Whether port has been modified since last sync.
    pub dirty: bool,
    /// Supported speeds (cached from SAI).
    pub supported_speeds: Vec<u32>,
    /// Supported FEC modes (cached from SAI).
    pub supported_fec_modes: Vec<PortFecMode>,
    /// Auxiliary data for vendor-specific extensions.
    pub aux_data: HashMap<String, String>,
}

impl Default for Port {
    fn default() -> Self {
        Self {
            // Identifiers
            alias: String::new(),
            description: String::new(),
            index: 0,
            port_type: PortType::default(),
            role: PortRole::default(),

            // SAI Object IDs
            port_id: 0,
            bridge_port_id: 0,
            vlan_member_id: 0,
            host_if_id: 0,
            rif_id: 0,
            ingress_acl_table_id: 0,
            egress_acl_table_id: 0,
            ingress_mirror_session_id: None,
            egress_mirror_session_id: None,
            ingress_sample_session_id: None,
            egress_sample_session_id: None,

            // Physical Properties
            lanes: Vec::new(),
            speed: 0,
            adv_speeds: Vec::new(),
            interface_type: PortInterfaceType::default(),
            adv_interface_types: Vec::new(),
            fec_mode: PortFecMode::default(),
            autoneg: PortAutoNegMode::default(),
            link_training: PortLinkTrainingMode::default(),
            mtu: 9100, // Default MTU
            tpid: 0x8100, // Default VLAN TPID
            preemphasis: None,
            serdes_lane_config: HashMap::new(),

            // Administrative State
            admin_state: PortAdminState::default(),
            oper_state: PortOperState::default(),
            initialized: false,

            // L2 Properties
            untagged_vlan: 1, // Default VLAN
            pvid: 1,
            mac_address: None,
            bridge_port_admin_state: true,
            learn_mode_set: false,

            // LAG Properties
            lag_id: None,
            lag_member_id: None,
            lag_members: HashSet::new(),

            // VLAN Properties
            vlan_id: 0,
            vlan_oid: None,
            vlan_members: HashSet::new(),

            // QoS Properties
            queue_ids: Vec::new(),
            num_unicast_queues: 0,
            num_multicast_queues: 0,
            num_queues: 0,
            priority_group_ids: Vec::new(),
            num_priority_groups: 0,
            scheduler_group_ids: Vec::new(),
            ingress_priority_group_index: HashMap::new(),
            queue_index_mapping: HashMap::new(),

            // ACL Properties
            acl_port_bind_mode: 0,
            ingress_acl_group_id: None,
            egress_acl_group_id: None,

            // PFC/Flow Control
            pfc_asym: false,
            pfc_priorities: 0,
            fc_tx_enable: false,
            fc_rx_enable: false,

            // Tunnel Properties
            tunnel_id: None,
            tunnel_type: None,

            // System Port Properties
            system_port_id: None,
            system_port_number: None,
            core_index: None,
            core_port_index: None,

            // Gearbox Properties
            gearbox_phy_id: None,
            gearbox_port_id: None,
            line_lanes: Vec::new(),
            system_lanes: Vec::new(),

            // Counters/Statistics
            debug_counter_enabled: false,
            counter_mode: 0,

            // Miscellaneous
            dirty: false,
            supported_speeds: Vec::new(),
            supported_fec_modes: Vec::new(),
            aux_data: HashMap::new(),
        }
    }
}

impl Port {
    /// Creates a new port with the given alias and type.
    pub fn new(alias: impl Into<String>, port_type: PortType) -> Self {
        Self {
            alias: alias.into(),
            port_type,
            ..Default::default()
        }
    }

    /// Creates a new physical port.
    pub fn physical(alias: impl Into<String>, lanes: Vec<u32>) -> Self {
        Self {
            alias: alias.into(),
            port_type: PortType::Phy,
            lanes,
            ..Default::default()
        }
    }

    /// Creates a new LAG port.
    pub fn lag(alias: impl Into<String>) -> Self {
        Self {
            alias: alias.into(),
            port_type: PortType::Lag,
            ..Default::default()
        }
    }

    /// Creates a new VLAN port (SVI).
    pub fn vlan(alias: impl Into<String>, vlan_id: u16) -> Self {
        Self {
            alias: alias.into(),
            port_type: PortType::Vlan,
            vlan_id,
            ..Default::default()
        }
    }

    /// Returns the SAI object ID for this port.
    pub fn sai_id(&self) -> RawSaiObjectId {
        self.port_id
    }

    /// Returns true if this is a physical port.
    pub fn is_physical(&self) -> bool {
        self.port_type == PortType::Phy
    }

    /// Returns true if this is a LAG port.
    pub fn is_lag(&self) -> bool {
        self.port_type == PortType::Lag
    }

    /// Returns true if this is a LAG member.
    pub fn is_lag_member(&self) -> bool {
        self.lag_id.is_some()
    }

    /// Returns true if this is a VLAN port (SVI).
    pub fn is_vlan(&self) -> bool {
        self.port_type == PortType::Vlan
    }

    /// Returns true if this port is admin up.
    pub fn is_admin_up(&self) -> bool {
        self.admin_state == PortAdminState::Up
    }

    /// Returns true if this port is operationally up.
    pub fn is_oper_up(&self) -> bool {
        self.oper_state == PortOperState::Up
    }

    /// Returns the number of lanes.
    pub fn lane_count(&self) -> usize {
        self.lanes.len()
    }

    /// Sets the port speed and marks dirty.
    pub fn set_speed(&mut self, speed: u32) {
        if self.speed != speed {
            self.speed = speed;
            self.dirty = true;
        }
    }

    /// Sets the admin state and marks dirty.
    pub fn set_admin_state(&mut self, state: PortAdminState) {
        if self.admin_state != state {
            self.admin_state = state;
            self.dirty = true;
        }
    }

    /// Sets the MTU and marks dirty.
    pub fn set_mtu(&mut self, mtu: u32) {
        if self.mtu != mtu {
            self.mtu = mtu;
            self.dirty = true;
        }
    }

    /// Sets the FEC mode and marks dirty.
    pub fn set_fec_mode(&mut self, fec: PortFecMode) {
        if self.fec_mode != fec {
            self.fec_mode = fec;
            self.dirty = true;
        }
    }

    /// Clears the dirty flag.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Returns true if the speed is supported.
    pub fn supports_speed(&self, speed: u32) -> bool {
        self.supported_speeds.is_empty() || self.supported_speeds.contains(&speed)
    }

    /// Returns true if the FEC mode is supported.
    pub fn supports_fec(&self, fec: PortFecMode) -> bool {
        self.supported_fec_modes.is_empty() || self.supported_fec_modes.contains(&fec)
    }

    /// Adds a LAG member to this port (must be a LAG).
    pub fn add_lag_member(&mut self, member_alias: impl Into<String>) {
        debug_assert!(self.is_lag(), "Cannot add LAG member to non-LAG port");
        self.lag_members.insert(member_alias.into());
    }

    /// Removes a LAG member from this port.
    pub fn remove_lag_member(&mut self, member_alias: &str) -> bool {
        self.lag_members.remove(member_alias)
    }

    /// Adds a VLAN membership.
    pub fn add_vlan_member(&mut self, vlan_id: u16) {
        self.vlan_members.insert(vlan_id);
    }

    /// Removes a VLAN membership.
    pub fn remove_vlan_member(&mut self, vlan_id: u16) -> bool {
        self.vlan_members.remove(&vlan_id)
    }
}

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Port({}, type={}, speed={}Mbps, admin={}, oper={})",
            self.alias, self.port_type, self.speed, self.admin_state, self.oper_state
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_type_parse() {
        assert_eq!("PHY".parse::<PortType>().unwrap(), PortType::Phy);
        assert_eq!("LAG".parse::<PortType>().unwrap(), PortType::Lag);
        assert_eq!("VLAN".parse::<PortType>().unwrap(), PortType::Vlan);
    }

    #[test]
    fn test_fec_mode_parse() {
        assert_eq!("none".parse::<PortFecMode>().unwrap(), PortFecMode::None);
        assert_eq!("rs".parse::<PortFecMode>().unwrap(), PortFecMode::Rs);
        assert_eq!("fc".parse::<PortFecMode>().unwrap(), PortFecMode::Fc);
    }

    #[test]
    fn test_port_physical() {
        let port = Port::physical("Ethernet0", vec![0, 1, 2, 3]);
        assert_eq!(port.alias, "Ethernet0");
        assert!(port.is_physical());
        assert_eq!(port.lane_count(), 4);
        assert!(!port.is_lag());
    }

    #[test]
    fn test_port_lag() {
        let mut port = Port::lag("PortChannel0001");
        assert!(port.is_lag());
        assert!(!port.is_physical());

        port.add_lag_member("Ethernet0");
        port.add_lag_member("Ethernet4");
        assert_eq!(port.lag_members.len(), 2);

        port.remove_lag_member("Ethernet0");
        assert_eq!(port.lag_members.len(), 1);
    }

    #[test]
    fn test_port_vlan() {
        let port = Port::vlan("Vlan100", 100);
        assert!(port.is_vlan());
        assert_eq!(port.vlan_id, 100);
    }

    #[test]
    fn test_port_dirty_tracking() {
        let mut port = Port::physical("Ethernet0", vec![0]);
        assert!(!port.dirty);

        port.set_speed(100000);
        assert!(port.dirty);

        port.clear_dirty();
        assert!(!port.dirty);

        // Setting same value shouldn't mark dirty
        port.set_speed(100000);
        assert!(!port.dirty);

        // Setting different value should mark dirty
        port.set_speed(25000);
        assert!(port.dirty);
    }

    #[test]
    fn test_port_admin_state() {
        let mut port = Port::default();
        assert!(!port.is_admin_up());

        port.set_admin_state(PortAdminState::Up);
        assert!(port.is_admin_up());
    }

    #[test]
    fn test_port_display() {
        let mut port = Port::physical("Ethernet0", vec![0, 1, 2, 3]);
        port.speed = 100000;
        port.admin_state = PortAdminState::Up;
        port.oper_state = PortOperState::Up;

        let display = port.to_string();
        assert!(display.contains("Ethernet0"));
        assert!(display.contains("PHY"));
        assert!(display.contains("100000Mbps"));
    }
}
