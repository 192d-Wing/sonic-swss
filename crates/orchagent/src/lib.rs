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
//! # Migration Status
//!
//! This crate is part of an ongoing migration from C++ to Rust. During the
//! migration period, it coexists with the C++ orchagent via FFI bridges.

pub mod acl;
pub mod bfd;
pub mod crm;
pub mod daemon;
pub mod debug_counter;
pub mod flex_counter;
pub mod isolation_group;
pub mod mlag;
pub mod nhg;
pub mod nvgre;
pub mod orch;
pub mod pfcwd;
pub mod policer;
pub mod ports;
pub mod route;
pub mod sflow;
pub mod stp;
pub mod tunnel_decap;
pub mod twamp;
pub mod vrf;
pub mod watermark;

// Re-export commonly used types
pub use sonic_orch_common::{
    Consumer, ConsumerConfig, KeyOpFieldsValues, Operation,
    Orch, OrchContext, TaskStatus, TaskResult,
    SyncMap, RetryCache, Constraint,
};
pub use sonic_sai::{SaiError, SaiResult, PortOid, SwitchOid};
pub use sonic_types::{MacAddress, IpAddress, IpPrefix, VlanId};

// Re-export FlexCounterOrch and related types
pub use flex_counter::{
    FlexCounterCallbacks, FlexCounterError, FlexCounterGroup, FlexCounterGroupMap,
    FlexCounterOrch, FlexCounterOrchConfig, FlexCounterPgStates, FlexCounterQueueStates,
    PgConfigurations, QueueConfigurations, register_flex_counter_orch, unregister_flex_counter_orch,
};

// Re-export RouteOrch and related types
pub use route::{
    NextHopFlags, NextHopGroupEntry, NextHopGroupKey, NextHopGroupTable, NextHopKey,
    RouteEntry, RouteError, RouteKey, RouteNhg, RouteOrch, RouteOrchCallbacks, RouteOrchConfig,
    RouteTables, register_route_orch, unregister_route_orch,
};

// Re-export PortsOrch and related types
pub use ports::{
    Port, PortAdminState, PortConfig, PortConfigError, PortFecMode, PortOperState,
    PortRole, PortsOrch, PortsOrchCallbacks, PortsOrchConfig, PortsOrchError, PortType,
    QueueInfo, QueueType, SchedulerInfo, register_ports_orch, unregister_ports_orch,
};

// Re-export AclOrch and related types
pub use acl::{
    AclActionType, AclBindPointType, AclMatchField, AclOrch, AclOrchCallbacks,
    AclOrchConfig, AclOrchError, AclPacketAction, AclPriority, AclRange,
    AclRangeType, AclRule, AclRuleAction, AclRuleId, AclRuleMatch, AclRuleType,
    AclStage, AclTable, AclTableConfig, AclTableId, AclTableType, AclTableTypeBuilder,
    MetaDataValue, register_acl_orch, unregister_acl_orch,
};

// Re-export VRFOrch and related types
pub use vrf::{
    L3VniEntry, VrfEntry, VrfId, VrfName, VrfOrch, VrfOrchCallbacks, VrfOrchConfig,
    VrfOrchError, VrfVlanId, Vni, register_vrf_orch, unregister_vrf_orch,
};

// Re-export WatermarkOrch and related types
pub use watermark::{
    ClearRequest, WatermarkGroup, WatermarkOrch, WatermarkOrchCallbacks,
    WatermarkOrchConfig, WatermarkOrchError, WatermarkStatus, WatermarkTable,
    register_watermark_orch, unregister_watermark_orch,
};

// Re-export CrmOrch and related types
pub use crm::{
    CrmOrch, CrmOrchCallbacks, CrmOrchConfig, CrmOrchError, CrmOrchStats,
    CrmResourceCounter, CrmResourceEntry, CrmResourceStatus, CrmResourceType,
    CrmThresholdField, CrmThresholdType, ThresholdCheck, register_crm_orch,
    unregister_crm_orch,
};

// Re-export MlagOrch and related types
pub use mlag::{
    MlagIfUpdate, MlagIslUpdate, MlagOrch, MlagOrchCallbacks, MlagOrchConfig,
    MlagOrchError, MlagOrchStats, MlagSubjectType, MlagUpdate, register_mlag_orch,
    unregister_mlag_orch,
};

// Re-export BfdOrch and related types
pub use bfd::{
    BfdOrch, BfdOrchCallbacks, BfdOrchConfig, BfdOrchError, BfdOrchStats,
    BfdSessionConfig, BfdSessionInfo, BfdSessionKey, BfdSessionState, BfdSessionType,
    BfdUpdate, register_bfd_orch, unregister_bfd_orch,
};

// Re-export SflowOrch and related types
pub use sflow::{
    PortSflowInfo, SampleDirection, SflowConfig, SflowOrch, SflowOrchCallbacks,
    SflowOrchConfig, SflowOrchError, SflowOrchStats, SflowSession,
    register_sflow_orch, unregister_sflow_orch,
};

// Re-export PolicerOrch and related types
pub use policer::{
    ColorSource, MeterType, PacketAction, PolicerConfig, PolicerEntry, PolicerMode,
    PolicerOrch, PolicerOrchCallbacks, PolicerOrchConfig, PolicerOrchError, PolicerOrchStats,
    StormType, register_policer_orch, unregister_policer_orch,
};

// Re-export StpOrch and related types
pub use stp::{
    SaiStpPortState, StpInstanceEntry, StpOrch, StpOrchCallbacks, StpOrchConfig,
    StpOrchError, StpOrchStats, StpPortIds, StpState, register_stp_orch, unregister_stp_orch,
};

// Re-export NvgreOrch and related types
pub use nvgre::{
    MapType, NvgreTunnel, NvgreTunnelConfig, NvgreTunnelMapConfig, NvgreTunnelMapEntry,
    NvgreOrch, NvgreOrchCallbacks, NvgreOrchConfig, NvgreOrchError, NvgreOrchStats,
    TunnelSaiIds, NVGRE_VSID_MAX_VALUE, register_nvgre_orch, unregister_nvgre_orch,
};

// Re-export IsolationGroupOrch and related types
pub use isolation_group::{
    IsolationGroupConfig, IsolationGroupEntry, IsolationGroupOrch, IsolationGroupOrchCallbacks,
    IsolationGroupOrchConfig, IsolationGroupOrchError, IsolationGroupOrchStats, IsolationGroupType,
    register_isolation_group_orch, unregister_isolation_group_orch,
};

// Re-export DebugCounterOrch and related types
pub use debug_counter::{
    DebugCounterConfig, DebugCounterEntry, DebugCounterOrch, DebugCounterOrchCallbacks,
    DebugCounterOrchConfig, DebugCounterOrchError, DebugCounterOrchStats, DebugCounterType,
    DropReason, FreeCounter, register_debug_counter_orch, unregister_debug_counter_orch,
};

// Re-export TwampOrch and related types
pub use twamp::{
    Dscp, SessionTimeout, TimestampFormat, TwampMode, TwampRole, TwampSessionConfig,
    TwampSessionEntry, TwampSessionStatus, TwampStats, TwampUdpPort, TxMode,
};

// Re-export PfcwdOrch and related types
pub use pfcwd::{
    DetectionTime, PfcWdAction, PfcWdConfig, PfcWdHwStats, PfcWdQueueEntry, RestorationTime,
};

// Re-export NhgOrch and related types
pub use nhg::{
    LabelStack, NextHopGroupMember, NhgEntry,
};

// Re-export TunnelDecapOrch and related types
pub use tunnel_decap::{
    EcnMode, NexthopTunnel, SubnetType, TunnelConfig, TunnelEntry, TunnelMode, TunnelTermEntry,
    TunnelTermType,
};
