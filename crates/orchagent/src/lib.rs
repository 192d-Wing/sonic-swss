//! SONiC Orchagent - Switch Orchestration Daemon
//!
//! This is the Rust implementation of the SONiC orchagent, responsible for
//! orchestrating switch configuration by translating high-level intent from
//! Redis databases into SAI API calls.
//!
//! # Architecture
//!
//! The orchagent follows an event-driven architecture:
//!
//! ```text
//! [CONFIG_DB] ─┐
//!              ├──> [OrchDaemon] ──> [SAI Redis] ──> [syncd] ──> [ASIC]
//! [APPL_DB] ───┘        │
//!                       ↓
//!                 [STATE_DB]
//! ```
//!
//! # Key Components
//!
//! - [`daemon::OrchDaemon`]: Main event loop and Orch coordination
//! - [`orch`]: Individual orchestration modules (PortsOrch, RouteOrch, etc.)
//!
//! # Feature Flags
//!
//! This crate supports feature flags to reduce build size and footprint:
//!
//! ## Deployment Profiles
//! - `full` (default): All features enabled
//! - `production`: Core + HA + telemetry (recommended)
//! - `production-minimal`: Core + essential HA
//! - `minimal`: Only core forwarding
//!
//! ## Feature Groups
//! - `core`: Essential forwarding (ports, routes, ACL, QoS, etc.)
//! - `telemetry`: Monitoring (flex-counter, watermark, sflow, etc.)
//! - `advanced-encapsulation`: Tunneling (VXLAN, NVGRE, SRv6, etc.)
//! - `security`: Security features (MACsec, NAT, PBH)
//! - `high-availability`: HA features (MLAG, STP, BFD, etc.)
//! - `port-specialization`: Advanced port features (fabric, isolation, etc.)
//!
//! # Migration Status
//!
//! This crate is part of an ongoing migration from C++ to Rust. During the
//! migration period, it coexists with the C++ orchagent via FFI bridges.

// ============================================================================
// Core Modules (essential for basic operation)
// ============================================================================

#[cfg(feature = "mod-acl")]
pub mod acl;
#[cfg(feature = "mod-audit")]
pub mod audit;
#[cfg(feature = "mod-buffer")]
pub mod buffer;
#[cfg(feature = "mod-daemon")]
pub mod daemon;
#[cfg(feature = "mod-fdb")]
pub mod fdb;
#[cfg(feature = "mod-fg-nhg")]
pub mod fg_nhg;
#[cfg(feature = "mod-icmp")]
pub mod icmp;
#[cfg(feature = "mod-intfs")]
pub mod intfs;
#[cfg(feature = "mod-neigh")]
pub mod neigh;
#[cfg(feature = "mod-nhg")]
pub mod nhg;
#[cfg(feature = "mod-orch")]
pub mod orch;
#[cfg(feature = "mod-policer")]
pub mod policer;
#[cfg(feature = "mod-ports")]
pub mod ports;
#[cfg(feature = "mod-qos")]
pub mod qos;
#[cfg(feature = "mod-route")]
pub mod route;
#[cfg(feature = "mod-switch")]
pub mod switch;
#[cfg(feature = "mod-vrf")]
pub mod vrf;
#[cfg(feature = "mod-zmq")]
pub mod zmq;

// ============================================================================
// Telemetry Modules
// ============================================================================

#[cfg(feature = "mod-countercheck")]
pub mod countercheck;
#[cfg(feature = "mod-crm")]
pub mod crm;
#[cfg(feature = "mod-debug-counter")]
pub mod debug_counter;
#[cfg(feature = "mod-dtel")]
pub mod dtel;
#[cfg(feature = "mod-flex-counter")]
pub mod flex_counter;
#[cfg(feature = "mod-sflow")]
pub mod sflow;
#[cfg(feature = "mod-twamp")]
pub mod twamp;
#[cfg(feature = "mod-watermark")]
pub mod watermark;

// ============================================================================
// Advanced Encapsulation Modules
// ============================================================================

#[cfg(feature = "mod-mplsroute")]
pub mod mplsroute;
#[cfg(feature = "mod-nvgre")]
pub mod nvgre;
#[cfg(feature = "mod-srv6")]
pub mod srv6;
#[cfg(feature = "mod-tunnel-decap")]
pub mod tunnel_decap;
#[cfg(feature = "mod-vnet")]
pub mod vnet;
#[cfg(feature = "mod-vxlan")]
pub mod vxlan;

// ============================================================================
// Security Modules
// ============================================================================

#[cfg(feature = "mod-macsec")]
pub mod macsec;
#[cfg(feature = "mod-nat")]
pub mod nat;
#[cfg(feature = "mod-pbh")]
pub mod pbh;

// ============================================================================
// High-Availability Modules
// ============================================================================

#[cfg(feature = "mod-bfd")]
pub mod bfd;
#[cfg(feature = "mod-chassis")]
pub mod chassis;
#[cfg(feature = "mod-mlag")]
pub mod mlag;
#[cfg(feature = "mod-mux")]
pub mod mux;
#[cfg(feature = "mod-pfcwd")]
pub mod pfcwd;
#[cfg(feature = "mod-stp")]
pub mod stp;

// ============================================================================
// Port Specialization Modules
// ============================================================================

#[cfg(feature = "mod-copp")]
pub mod copp;
#[cfg(feature = "mod-fabric-ports")]
pub mod fabric_ports;
#[cfg(feature = "mod-isolation-group")]
pub mod isolation_group;
#[cfg(feature = "mod-mirror")]
pub mod mirror;

// ============================================================================
// Re-exports
// ============================================================================

// Re-export commonly used types (always available)
pub use sonic_orch_common::{
    Constraint, Consumer, ConsumerConfig, KeyOpFieldsValues, Operation, Orch, OrchContext,
    RetryCache, SyncMap, TaskResult, TaskStatus,
};
pub use sonic_sai::{PortOid, SaiError, SaiResult, SwitchOid};
pub use sonic_types::{IpAddress, IpPrefix, MacAddress, VlanId};

// ============================================================================
// Core Module Re-exports
// ============================================================================

#[cfg(feature = "mod-flex-counter")]
pub use flex_counter::{
    register_flex_counter_orch, unregister_flex_counter_orch, FlexCounterCallbacks,
    FlexCounterError, FlexCounterGroup, FlexCounterGroupMap, FlexCounterOrch,
    FlexCounterOrchConfig, FlexCounterPgStates, FlexCounterQueueStates, PgConfigurations,
    QueueConfigurations,
};

#[cfg(feature = "mod-route")]
pub use route::{
    register_route_orch, unregister_route_orch, NextHopFlags, NextHopGroupEntry, NextHopGroupKey,
    NextHopGroupTable, NextHopKey, RouteEntry, RouteError, RouteKey, RouteNhg, RouteOrch,
    RouteOrchCallbacks, RouteOrchConfig, RouteTables,
};

#[cfg(feature = "mod-ports")]
pub use ports::{
    register_ports_orch, unregister_ports_orch, Port, PortAdminState, PortConfig, PortConfigError,
    PortFecMode, PortOperState, PortRole, PortType, PortsOrch, PortsOrchCallbacks, PortsOrchConfig,
    PortsOrchError, QueueInfo, QueueType, SchedulerInfo, VlanTaggingMode,
};

pub use intfs::{
    register_intfs_orch, unregister_intfs_orch, IntfsEntry, IntfsOrch, IntfsOrchCallbacks,
    IntfsOrchConfig, IntfsOrchError, IntfsOrchStats, RifType,
};

#[cfg(feature = "mod-acl")]
pub use acl::{
    register_acl_orch, unregister_acl_orch, AclActionType, AclBindPointType, AclMatchField,
    AclMatchValue, AclOrch, AclOrchCallbacks, AclOrchConfig, AclOrchError, AclPacketAction,
    AclPriority, AclRange, AclRangeType, AclRedirectTarget, AclRule, AclRuleAction, AclRuleId,
    AclRuleMatch, AclRuleType, AclStage, AclTable, AclTableConfig, AclTableId, AclTableType,
    AclTableTypeBuilder, MetaDataValue,
};

#[cfg(feature = "mod-vrf")]
pub use vrf::{
    register_vrf_orch, unregister_vrf_orch, L3VniEntry, Vni, VrfEntry, VrfId, VrfName, VrfOrch,
    VrfOrchCallbacks, VrfOrchConfig, VrfOrchError, VrfVlanId,
};

#[cfg(feature = "mod-policer")]
pub use policer::{
    register_policer_orch, unregister_policer_orch, ColorSource, MeterType, PacketAction,
    PolicerConfig, PolicerEntry, PolicerMode, PolicerOrch, PolicerOrchCallbacks, PolicerOrchConfig,
    PolicerOrchError, PolicerOrchStats, StormType,
};

#[cfg(feature = "mod-nhg")]
pub use nhg::{LabelStack, NextHopGroupMember, NhgEntry};

// ============================================================================
// Telemetry Module Re-exports
// ============================================================================

#[cfg(feature = "mod-watermark")]
pub use watermark::{
    register_watermark_orch, unregister_watermark_orch, ClearRequest, WatermarkGroup,
    WatermarkOrch, WatermarkOrchCallbacks, WatermarkOrchConfig, WatermarkOrchError,
    WatermarkStatus, WatermarkTable,
};

#[cfg(feature = "mod-crm")]
pub use crm::{
    register_crm_orch, unregister_crm_orch, CrmOrch, CrmOrchCallbacks, CrmOrchConfig, CrmOrchError,
    CrmOrchStats, CrmResourceCounter, CrmResourceEntry, CrmResourceStatus, CrmResourceType,
    CrmThresholdField, CrmThresholdType, ThresholdCheck,
};

#[cfg(feature = "mod-sflow")]
pub use sflow::{
    register_sflow_orch, unregister_sflow_orch, PortSflowInfo, SampleDirection, SflowConfig,
    SflowOrch, SflowOrchCallbacks, SflowOrchConfig, SflowOrchError, SflowOrchStats, SflowSession,
};

#[cfg(feature = "mod-debug-counter")]
pub use debug_counter::{
    register_debug_counter_orch, unregister_debug_counter_orch, DebugCounterConfig,
    DebugCounterEntry, DebugCounterOrch, DebugCounterOrchCallbacks, DebugCounterOrchConfig,
    DebugCounterOrchError, DebugCounterOrchStats, DebugCounterType, DropReason, FreeCounter,
};

#[cfg(feature = "mod-twamp")]
pub use twamp::{
    Dscp, SessionTimeout, TimestampFormat, TwampMode, TwampRole, TwampSessionConfig,
    TwampSessionEntry, TwampSessionStatus, TwampStats, TwampUdpPort, TxMode,
};

// ============================================================================
// High-Availability Module Re-exports
// ============================================================================

#[cfg(feature = "mod-mlag")]
pub use mlag::{
    register_mlag_orch, unregister_mlag_orch, MlagIfUpdate, MlagIslUpdate, MlagOrch,
    MlagOrchCallbacks, MlagOrchConfig, MlagOrchError, MlagOrchStats, MlagSubjectType, MlagUpdate,
};

#[cfg(feature = "mod-bfd")]
pub use bfd::{
    register_bfd_orch, unregister_bfd_orch, BfdOrch, BfdOrchCallbacks, BfdOrchConfig, BfdOrchError,
    BfdOrchStats, BfdSessionConfig, BfdSessionInfo, BfdSessionKey, BfdSessionState, BfdSessionType,
    BfdUpdate,
};

#[cfg(feature = "mod-stp")]
pub use stp::{
    register_stp_orch, unregister_stp_orch, SaiStpPortState, StpInstanceEntry, StpOrch,
    StpOrchCallbacks, StpOrchConfig, StpOrchError, StpOrchStats, StpPortIds, StpState,
};

#[cfg(feature = "mod-pfcwd")]
pub use pfcwd::{
    DetectionTime, PfcWdAction, PfcWdConfig, PfcWdHwStats, PfcWdQueueEntry, RestorationTime,
};

// ============================================================================
// Advanced Encapsulation Module Re-exports
// ============================================================================

#[cfg(feature = "mod-nvgre")]
pub use nvgre::{
    register_nvgre_orch, unregister_nvgre_orch, MapType, NvgreOrch, NvgreOrchCallbacks,
    NvgreOrchConfig, NvgreOrchError, NvgreOrchStats, NvgreTunnel, NvgreTunnelConfig,
    NvgreTunnelMapConfig, NvgreTunnelMapEntry, TunnelSaiIds, NVGRE_VSID_MAX_VALUE,
};

#[cfg(feature = "mod-tunnel-decap")]
pub use tunnel_decap::{
    EcnMode, NexthopTunnel, SubnetType, TunnelConfig, TunnelEntry, TunnelMode, TunnelTermEntry,
    TunnelTermType,
};

// ============================================================================
// Port Specialization Module Re-exports
// ============================================================================

#[cfg(feature = "mod-isolation-group")]
pub use isolation_group::{
    register_isolation_group_orch, unregister_isolation_group_orch, IsolationGroupConfig,
    IsolationGroupEntry, IsolationGroupOrch, IsolationGroupOrchCallbacks, IsolationGroupOrchConfig,
    IsolationGroupOrchError, IsolationGroupOrchStats, IsolationGroupType,
};
