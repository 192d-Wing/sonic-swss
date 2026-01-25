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
pub mod audit;
pub mod bfd;
pub mod buffer;
pub mod chassis;
pub mod copp;
pub mod countercheck;
pub mod crm;
pub mod daemon;
pub mod debug_counter;
pub mod dtel;
pub mod fabric_ports;
pub mod fdb;
pub mod fg_nhg;
pub mod flex_counter;
pub mod icmp;
pub mod intfs;
pub mod isolation_group;
pub mod macsec;
pub mod mirror;
pub mod mlag;
pub mod mplsroute;
pub mod mux;
pub mod nat;
pub mod neigh;
pub mod nhg;
pub mod nvgre;
pub mod orch;
pub mod pbh;
pub mod pfcwd;
pub mod policer;
pub mod ports;
pub mod qos;
pub mod route;
pub mod sflow;
pub mod srv6;
pub mod stp;
pub mod switch;
pub mod tunnel_decap;
pub mod twamp;
pub mod vnet;
pub mod vrf;
pub mod vxlan;
pub mod watermark;
pub mod zmq;

// Re-export commonly used types
pub use sonic_orch_common::{
    Constraint, Consumer, ConsumerConfig, KeyOpFieldsValues, Operation, Orch, OrchContext,
    RetryCache, SyncMap, TaskResult, TaskStatus,
};
pub use sonic_sai::{PortOid, SaiError, SaiResult, SwitchOid};
pub use sonic_types::{IpAddress, IpPrefix, MacAddress, VlanId};

// Re-export FlexCounterOrch and related types
pub use flex_counter::{
    register_flex_counter_orch, unregister_flex_counter_orch, FlexCounterCallbacks,
    FlexCounterError, FlexCounterGroup, FlexCounterGroupMap, FlexCounterOrch,
    FlexCounterOrchConfig, FlexCounterPgStates, FlexCounterQueueStates, PgConfigurations,
    QueueConfigurations,
};

// Re-export RouteOrch and related types
pub use route::{
    register_route_orch, unregister_route_orch, NextHopFlags, NextHopGroupEntry, NextHopGroupKey,
    NextHopGroupTable, NextHopKey, RouteEntry, RouteError, RouteKey, RouteNhg, RouteOrch,
    RouteOrchCallbacks, RouteOrchConfig, RouteTables,
};

// Re-export PortsOrch and related types
pub use ports::{
    register_ports_orch, unregister_ports_orch, Port, PortAdminState, PortConfig, PortConfigError,
    PortFecMode, PortOperState, PortRole, PortType, PortsOrch, PortsOrchCallbacks, PortsOrchConfig,
    PortsOrchError, QueueInfo, QueueType, SchedulerInfo, VlanTaggingMode,
};

// Re-export AclOrch and related types
pub use acl::{
    register_acl_orch, unregister_acl_orch, AclActionType, AclBindPointType, AclMatchField,
    AclMatchValue, AclOrch, AclOrchCallbacks, AclOrchConfig, AclOrchError, AclPacketAction,
    AclPriority, AclRange, AclRangeType, AclRedirectTarget, AclRule, AclRuleAction, AclRuleId,
    AclRuleMatch, AclRuleType, AclStage, AclTable, AclTableConfig, AclTableId, AclTableType,
    AclTableTypeBuilder, MetaDataValue,
};

// Re-export VRFOrch and related types
pub use vrf::{
    register_vrf_orch, unregister_vrf_orch, L3VniEntry, Vni, VrfEntry, VrfId, VrfName, VrfOrch,
    VrfOrchCallbacks, VrfOrchConfig, VrfOrchError, VrfVlanId,
};

// Re-export WatermarkOrch and related types
pub use watermark::{
    register_watermark_orch, unregister_watermark_orch, ClearRequest, WatermarkGroup,
    WatermarkOrch, WatermarkOrchCallbacks, WatermarkOrchConfig, WatermarkOrchError,
    WatermarkStatus, WatermarkTable,
};

// Re-export CrmOrch and related types
pub use crm::{
    register_crm_orch, unregister_crm_orch, CrmOrch, CrmOrchCallbacks, CrmOrchConfig, CrmOrchError,
    CrmOrchStats, CrmResourceCounter, CrmResourceEntry, CrmResourceStatus, CrmResourceType,
    CrmThresholdField, CrmThresholdType, ThresholdCheck,
};

// Re-export MlagOrch and related types
pub use mlag::{
    register_mlag_orch, unregister_mlag_orch, MlagIfUpdate, MlagIslUpdate, MlagOrch,
    MlagOrchCallbacks, MlagOrchConfig, MlagOrchError, MlagOrchStats, MlagSubjectType, MlagUpdate,
};

// Re-export BfdOrch and related types
pub use bfd::{
    register_bfd_orch, unregister_bfd_orch, BfdOrch, BfdOrchCallbacks, BfdOrchConfig, BfdOrchError,
    BfdOrchStats, BfdSessionConfig, BfdSessionInfo, BfdSessionKey, BfdSessionState, BfdSessionType,
    BfdUpdate,
};

// Re-export SflowOrch and related types
pub use sflow::{
    register_sflow_orch, unregister_sflow_orch, PortSflowInfo, SampleDirection, SflowConfig,
    SflowOrch, SflowOrchCallbacks, SflowOrchConfig, SflowOrchError, SflowOrchStats, SflowSession,
};

// Re-export PolicerOrch and related types
pub use policer::{
    register_policer_orch, unregister_policer_orch, ColorSource, MeterType, PacketAction,
    PolicerConfig, PolicerEntry, PolicerMode, PolicerOrch, PolicerOrchCallbacks, PolicerOrchConfig,
    PolicerOrchError, PolicerOrchStats, StormType,
};

// Re-export StpOrch and related types
pub use stp::{
    register_stp_orch, unregister_stp_orch, SaiStpPortState, StpInstanceEntry, StpOrch,
    StpOrchCallbacks, StpOrchConfig, StpOrchError, StpOrchStats, StpPortIds, StpState,
};

// Re-export NvgreOrch and related types
pub use nvgre::{
    register_nvgre_orch, unregister_nvgre_orch, MapType, NvgreOrch, NvgreOrchCallbacks,
    NvgreOrchConfig, NvgreOrchError, NvgreOrchStats, NvgreTunnel, NvgreTunnelConfig,
    NvgreTunnelMapConfig, NvgreTunnelMapEntry, TunnelSaiIds, NVGRE_VSID_MAX_VALUE,
};

// Re-export IsolationGroupOrch and related types
pub use isolation_group::{
    register_isolation_group_orch, unregister_isolation_group_orch, IsolationGroupConfig,
    IsolationGroupEntry, IsolationGroupOrch, IsolationGroupOrchCallbacks, IsolationGroupOrchConfig,
    IsolationGroupOrchError, IsolationGroupOrchStats, IsolationGroupType,
};

// Re-export DebugCounterOrch and related types
pub use debug_counter::{
    register_debug_counter_orch, unregister_debug_counter_orch, DebugCounterConfig,
    DebugCounterEntry, DebugCounterOrch, DebugCounterOrchCallbacks, DebugCounterOrchConfig,
    DebugCounterOrchError, DebugCounterOrchStats, DebugCounterType, DropReason, FreeCounter,
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
pub use nhg::{LabelStack, NextHopGroupMember, NhgEntry};

// Re-export TunnelDecapOrch and related types
pub use tunnel_decap::{
    EcnMode, NexthopTunnel, SubnetType, TunnelConfig, TunnelEntry, TunnelMode, TunnelTermEntry,
    TunnelTermType,
};
