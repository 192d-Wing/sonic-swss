//! Type-safe SAI object ID wrappers.
//!
//! This module provides strongly-typed wrappers for SAI object IDs, preventing
//! accidental mixing of different object types (e.g., passing a port OID where
//! a route OID is expected).

use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;

/// Raw SAI object ID type (matches sai_object_id_t in C).
pub type RawSaiObjectId = u64;

/// Marker trait for SAI object kinds.
///
/// Each SAI object type implements this trait to enable compile-time
/// type checking of object IDs.
pub trait SaiObjectKind: Send + Sync + 'static {
    /// Returns the SAI object type name for debugging.
    fn type_name() -> &'static str;
}

/// A type-safe SAI object ID.
///
/// This wrapper ensures that object IDs of different types cannot be
/// accidentally mixed. The phantom type parameter `T` indicates what
/// kind of SAI object this ID refers to.
///
/// # Examples
///
/// ```
/// use sonic_sai::{PortOid, NextHopOid, SaiObjectId};
///
/// // Different OID types are incompatible at compile time
/// let port: PortOid = PortOid::from_raw(0x1000000000001).unwrap();
/// let nhop: NextHopOid = NextHopOid::from_raw(0x4000000000001).unwrap();
///
/// // This would fail to compile:
/// // fn takes_port(p: PortOid) {}
/// // takes_port(nhop);  // Error: expected PortOid, found NextHopOid
/// ```
#[derive(Clone, Copy)]
pub struct SaiObjectId<T: SaiObjectKind> {
    raw: RawSaiObjectId,
    _marker: PhantomData<T>,
}

impl<T: SaiObjectKind> SaiObjectId<T> {
    /// The null object ID (SAI_NULL_OBJECT_ID).
    pub const NULL: Self = Self {
        raw: 0,
        _marker: PhantomData,
    };

    /// Creates a new object ID from a raw value.
    ///
    /// Returns `None` if the raw value is 0 (null object ID).
    /// Use `NULL` constant for explicitly null IDs.
    pub fn from_raw(raw: RawSaiObjectId) -> Option<Self> {
        if raw == 0 {
            None
        } else {
            Some(Self {
                raw,
                _marker: PhantomData,
            })
        }
    }

    /// Creates a new object ID from a raw value, including null.
    ///
    /// Unlike `from_raw`, this allows creating null object IDs.
    pub const fn from_raw_unchecked(raw: RawSaiObjectId) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }

    /// Returns the raw object ID value.
    pub const fn as_raw(&self) -> RawSaiObjectId {
        self.raw
    }

    /// Returns true if this is a null object ID.
    pub const fn is_null(&self) -> bool {
        self.raw == 0
    }

    /// Returns true if this is a valid (non-null) object ID.
    pub const fn is_valid(&self) -> bool {
        self.raw != 0
    }
}

impl<T: SaiObjectKind> fmt::Debug for SaiObjectId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(0x{:016x})", T::type_name(), self.raw)
    }
}

impl<T: SaiObjectKind> fmt::Display for SaiObjectId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:016x}", self.raw)
    }
}

impl<T: SaiObjectKind> PartialEq for SaiObjectId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<T: SaiObjectKind> Eq for SaiObjectId<T> {}

impl<T: SaiObjectKind> Hash for SaiObjectId<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<T: SaiObjectKind> Default for SaiObjectId<T> {
    fn default() -> Self {
        Self::NULL
    }
}

// ============================================================================
// Object Kind Markers
// ============================================================================

macro_rules! define_object_kind {
    ($name:ident, $type_name:literal, $oid_alias:ident) => {
        /// Marker type for SAI $type_name objects.
        #[derive(Debug, Clone, Copy)]
        pub struct $name;

        impl SaiObjectKind for $name {
            fn type_name() -> &'static str {
                $type_name
            }
        }

        /// Type alias for $type_name object IDs.
        pub type $oid_alias = SaiObjectId<$name>;
    };
}

// Define all SAI object types
define_object_kind!(SwitchKind, "Switch", SwitchOid);
define_object_kind!(PortKind, "Port", PortOid);
define_object_kind!(VirtualRouterKind, "VirtualRouter", VirtualRouterOid);
define_object_kind!(RouterInterfaceKind, "RouterInterface", RouterInterfaceOid);
define_object_kind!(NextHopKind, "NextHop", NextHopOid);
define_object_kind!(NextHopGroupKind, "NextHopGroup", NextHopGroupOid);
define_object_kind!(NextHopGroupMemberKind, "NextHopGroupMember", NextHopGroupMemberOid);
define_object_kind!(AclTableKind, "AclTable", AclTableOid);
define_object_kind!(AclEntryKind, "AclEntry", AclEntryOid);
define_object_kind!(AclCounterKind, "AclCounter", AclCounterOid);
define_object_kind!(VlanKind, "Vlan", VlanOid);
define_object_kind!(VlanMemberKind, "VlanMember", VlanMemberOid);
define_object_kind!(LagKind, "Lag", LagOid);
define_object_kind!(LagMemberKind, "LagMember", LagMemberOid);
define_object_kind!(BridgeKind, "Bridge", BridgeOid);
define_object_kind!(BridgePortKind, "BridgePort", BridgePortOid);
define_object_kind!(FdbEntryKind, "FdbEntry", FdbEntryOid);
define_object_kind!(NeighborEntryKind, "NeighborEntry", NeighborEntryOid);
define_object_kind!(RouteEntryKind, "RouteEntry", RouteEntryOid);
define_object_kind!(BufferPoolKind, "BufferPool", BufferPoolOid);
define_object_kind!(BufferProfileKind, "BufferProfile", BufferProfileOid);
define_object_kind!(QueueKind, "Queue", QueueOid);
define_object_kind!(SchedulerKind, "Scheduler", SchedulerOid);
define_object_kind!(SchedulerGroupKind, "SchedulerGroup", SchedulerGroupOid);
define_object_kind!(IngressPriorityGroupKind, "IngressPriorityGroup", IngressPriorityGroupOid);
define_object_kind!(TunnelKind, "Tunnel", TunnelOid);
define_object_kind!(TunnelMapKind, "TunnelMap", TunnelMapOid);
define_object_kind!(TunnelMapEntryKind, "TunnelMapEntry", TunnelMapEntryOid);
define_object_kind!(TunnelTermKind, "TunnelTerm", TunnelTermOid);
define_object_kind!(MirrorSessionKind, "MirrorSession", MirrorSessionOid);
define_object_kind!(PolicerKind, "Policer", PolicerOid);
define_object_kind!(WredKind, "Wred", WredOid);
define_object_kind!(QosMapKind, "QosMap", QosMapOid);
define_object_kind!(HostifKind, "Hostif", HostifOid);
define_object_kind!(HostifTrapKind, "HostifTrap", HostifTrapOid);
define_object_kind!(HostifTrapGroupKind, "HostifTrapGroup", HostifTrapGroupOid);
define_object_kind!(HashKind, "Hash", HashOid);
define_object_kind!(SamplePacketKind, "SamplePacket", SamplePacketOid);
define_object_kind!(CounterKind, "Counter", CounterOid);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oid_creation() {
        let port = PortOid::from_raw(0x1000000000001).unwrap();
        assert_eq!(port.as_raw(), 0x1000000000001);
        assert!(port.is_valid());
        assert!(!port.is_null());
    }

    #[test]
    fn test_null_oid() {
        assert!(PortOid::from_raw(0).is_none());
        assert!(PortOid::NULL.is_null());
        assert!(!PortOid::NULL.is_valid());
    }

    #[test]
    fn test_oid_debug() {
        let port = PortOid::from_raw(0x1000000000001).unwrap();
        let debug = format!("{:?}", port);
        assert!(debug.contains("Port"));
        assert!(debug.contains("0x0001000000000001"));
    }

    #[test]
    fn test_oid_equality() {
        let p1 = PortOid::from_raw(0x1000000000001).unwrap();
        let p2 = PortOid::from_raw(0x1000000000001).unwrap();
        let p3 = PortOid::from_raw(0x1000000000002).unwrap();

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_type_safety() {
        // This test verifies that different OID types are incompatible
        // The actual compile-time check is done by the type system
        let _port: PortOid = PortOid::from_raw(1).unwrap();
        let _nhop: NextHopOid = NextHopOid::from_raw(2).unwrap();

        // These are different types and cannot be compared directly
        // (unless we impl PartialEq between them, which we don't)
    }
}
