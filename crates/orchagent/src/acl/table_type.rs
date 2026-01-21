//! ACL table type definitions and builder.
//!
//! An ACL table type defines what match fields and actions a table supports,
//! as well as what bind points it can be attached to.

use std::collections::HashSet;
use std::fmt;

use super::types::{AclActionType, AclBindPointType, AclMatchField, AclStage};

/// ACL table type definition.
///
/// A table type defines the capabilities of an ACL table:
/// - What match fields are supported
/// - What actions are supported
/// - What bind points it can be attached to (ports, LAGs, switch)
#[derive(Debug, Clone)]
pub struct AclTableType {
    /// Type name (e.g., "L3", "MIRROR").
    pub name: String,
    /// Supported bind point types.
    pub bind_points: HashSet<AclBindPointType>,
    /// Supported match fields.
    pub matches: HashSet<AclMatchField>,
    /// Supported actions.
    pub actions: HashSet<AclActionType>,
    /// Supported stages.
    pub stages: HashSet<AclStage>,
    /// Whether this is a built-in type.
    pub is_builtin: bool,
}

impl AclTableType {
    /// Creates a new empty table type with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            bind_points: HashSet::new(),
            matches: HashSet::new(),
            actions: HashSet::new(),
            stages: HashSet::new(),
            is_builtin: false,
        }
    }

    /// Returns true if this type supports the given match field.
    pub fn supports_match(&self, field: AclMatchField) -> bool {
        self.matches.contains(&field)
    }

    /// Returns true if this type supports the given action.
    pub fn supports_action(&self, action: AclActionType) -> bool {
        self.actions.contains(&action)
    }

    /// Returns true if this type supports the given bind point.
    pub fn supports_bind_point(&self, bp: AclBindPointType) -> bool {
        self.bind_points.contains(&bp)
    }

    /// Returns true if this type supports the given stage.
    pub fn supports_stage(&self, stage: AclStage) -> bool {
        // If no stages specified, support both
        self.stages.is_empty() || self.stages.contains(&stage)
    }

    /// Validates that a rule can be created with this table type.
    pub fn validate_matches(&self, matches: &HashSet<AclMatchField>) -> Result<(), String> {
        for field in matches {
            if !self.supports_match(*field) {
                return Err(format!(
                    "Table type {} does not support match field {}",
                    self.name, field
                ));
            }
        }
        Ok(())
    }

    /// Validates that a rule's actions are supported by this table type.
    pub fn validate_actions(&self, actions: &HashSet<AclActionType>) -> Result<(), String> {
        for action in actions {
            if !self.supports_action(*action) {
                return Err(format!(
                    "Table type {} does not support action {}",
                    self.name, action
                ));
            }
        }
        Ok(())
    }
}

impl fmt::Display for AclTableType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AclTableType({}, matches={}, actions={}, bind_points={})",
            self.name,
            self.matches.len(),
            self.actions.len(),
            self.bind_points.len()
        )
    }
}

/// Builder for ACL table types.
///
/// Uses a fluent API for constructing table type definitions.
#[derive(Debug, Clone, Default)]
pub struct AclTableTypeBuilder {
    name: Option<String>,
    bind_points: HashSet<AclBindPointType>,
    matches: HashSet<AclMatchField>,
    actions: HashSet<AclActionType>,
    stages: HashSet<AclStage>,
    is_builtin: bool,
}

impl AclTableTypeBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the type name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Adds a bind point type.
    pub fn with_bind_point(mut self, bp: AclBindPointType) -> Self {
        self.bind_points.insert(bp);
        self
    }

    /// Adds multiple bind point types.
    pub fn with_bind_points(mut self, bps: impl IntoIterator<Item = AclBindPointType>) -> Self {
        self.bind_points.extend(bps);
        self
    }

    /// Adds a match field.
    pub fn with_match(mut self, field: AclMatchField) -> Self {
        self.matches.insert(field);
        self
    }

    /// Adds multiple match fields.
    pub fn with_matches(mut self, fields: impl IntoIterator<Item = AclMatchField>) -> Self {
        self.matches.extend(fields);
        self
    }

    /// Adds an action type.
    pub fn with_action(mut self, action: AclActionType) -> Self {
        self.actions.insert(action);
        self
    }

    /// Adds multiple action types.
    pub fn with_actions(mut self, actions: impl IntoIterator<Item = AclActionType>) -> Self {
        self.actions.extend(actions);
        self
    }

    /// Adds a supported stage.
    pub fn with_stage(mut self, stage: AclStage) -> Self {
        self.stages.insert(stage);
        self
    }

    /// Sets this as a built-in type.
    pub fn builtin(mut self) -> Self {
        self.is_builtin = true;
        self
    }

    /// Builds the table type.
    pub fn build(self) -> Result<AclTableType, String> {
        let name = self.name.ok_or("Table type name is required")?;

        if self.bind_points.is_empty() {
            return Err("At least one bind point is required".to_string());
        }

        if self.matches.is_empty() && self.actions.is_empty() {
            return Err("At least one match or action is required".to_string());
        }

        Ok(AclTableType {
            name,
            bind_points: self.bind_points,
            matches: self.matches,
            actions: self.actions,
            stages: self.stages,
            is_builtin: self.is_builtin,
        })
    }
}

/// Creates the built-in L3 table type.
pub fn create_l3_table_type() -> AclTableType {
    AclTableTypeBuilder::new()
        .with_name("L3")
        .with_bind_points([AclBindPointType::Port, AclBindPointType::Lag])
        .with_matches([
            AclMatchField::SrcIp,
            AclMatchField::DstIp,
            AclMatchField::EtherType,
            AclMatchField::IpProtocol,
            AclMatchField::Dscp,
            AclMatchField::TcpFlags,
            AclMatchField::IcmpType,
            AclMatchField::IcmpCode,
            AclMatchField::L4SrcPort,
            AclMatchField::L4DstPort,
            AclMatchField::L4SrcPortRange,
            AclMatchField::L4DstPortRange,
            AclMatchField::InPorts,
        ])
        .with_actions([
            AclActionType::PacketAction,
            AclActionType::Redirect,
            AclActionType::Counter,
        ])
        .builtin()
        .build()
        .expect("L3 table type should be valid")
}

/// Creates the built-in L3V6 table type.
pub fn create_l3v6_table_type() -> AclTableType {
    AclTableTypeBuilder::new()
        .with_name("L3V6")
        .with_bind_points([AclBindPointType::Port, AclBindPointType::Lag])
        .with_matches([
            AclMatchField::SrcIpv6,
            AclMatchField::DstIpv6,
            AclMatchField::EtherType,
            AclMatchField::Ipv6NextHeader,
            AclMatchField::Dscp,
            AclMatchField::TcpFlags,
            AclMatchField::Icmpv6Type,
            AclMatchField::Icmpv6Code,
            AclMatchField::L4SrcPort,
            AclMatchField::L4DstPort,
            AclMatchField::L4SrcPortRange,
            AclMatchField::L4DstPortRange,
            AclMatchField::InPorts,
        ])
        .with_actions([
            AclActionType::PacketAction,
            AclActionType::Redirect,
            AclActionType::Counter,
        ])
        .builtin()
        .build()
        .expect("L3V6 table type should be valid")
}

/// Creates the built-in MIRROR table type.
pub fn create_mirror_table_type() -> AclTableType {
    AclTableTypeBuilder::new()
        .with_name("MIRROR")
        .with_bind_points([AclBindPointType::Port, AclBindPointType::Lag])
        .with_matches([
            AclMatchField::SrcIp,
            AclMatchField::DstIp,
            AclMatchField::EtherType,
            AclMatchField::IpProtocol,
            AclMatchField::Dscp,
            AclMatchField::TcpFlags,
            AclMatchField::L4SrcPort,
            AclMatchField::L4DstPort,
            AclMatchField::InPorts,
        ])
        .with_actions([
            AclActionType::MirrorIngress,
            AclActionType::MirrorEgress,
            AclActionType::Counter,
        ])
        .builtin()
        .build()
        .expect("MIRROR table type should be valid")
}

/// Creates the built-in PFCWD table type.
pub fn create_pfcwd_table_type() -> AclTableType {
    AclTableTypeBuilder::new()
        .with_name("PFCWD")
        .with_bind_points([AclBindPointType::Port, AclBindPointType::Switch])
        .with_matches([AclMatchField::Tc, AclMatchField::InPorts])
        .with_actions([AclActionType::PacketAction, AclActionType::Counter])
        .with_stage(AclStage::Ingress)
        .builtin()
        .build()
        .expect("PFCWD table type should be valid")
}

/// Creates the built-in DROP table type.
pub fn create_drop_table_type() -> AclTableType {
    AclTableTypeBuilder::new()
        .with_name("DROP")
        .with_bind_points([AclBindPointType::Port, AclBindPointType::Lag])
        .with_matches([
            AclMatchField::SrcIp,
            AclMatchField::DstIp,
            AclMatchField::SrcIpv6,
            AclMatchField::DstIpv6,
            AclMatchField::EtherType,
            AclMatchField::IpProtocol,
            AclMatchField::Ipv6NextHeader,
            AclMatchField::L4SrcPort,
            AclMatchField::L4DstPort,
            AclMatchField::InPorts,
        ])
        .with_actions([AclActionType::PacketAction, AclActionType::Counter])
        .with_stage(AclStage::Ingress)
        .builtin()
        .build()
        .expect("DROP table type should be valid")
}

/// Creates the built-in CTRLPLANE table type.
pub fn create_ctrlplane_table_type() -> AclTableType {
    AclTableTypeBuilder::new()
        .with_name("CTRLPLANE")
        .with_bind_points([AclBindPointType::Port, AclBindPointType::Lag])
        .with_matches([
            AclMatchField::SrcIp,
            AclMatchField::DstIp,
            AclMatchField::SrcIpv6,
            AclMatchField::DstIpv6,
            AclMatchField::EtherType,
            AclMatchField::IpProtocol,
            AclMatchField::Ipv6NextHeader,
            AclMatchField::L4SrcPort,
            AclMatchField::L4DstPort,
            AclMatchField::InPorts,
        ])
        .with_actions([AclActionType::PacketAction, AclActionType::Counter])
        .with_stage(AclStage::Ingress)
        .builtin()
        .build()
        .expect("CTRLPLANE table type should be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_type_new() {
        let tt = AclTableType::new("TEST");
        assert_eq!(tt.name, "TEST");
        assert!(tt.matches.is_empty());
        assert!(tt.actions.is_empty());
        assert!(tt.bind_points.is_empty());
    }

    #[test]
    fn test_table_type_builder() {
        let tt = AclTableTypeBuilder::new()
            .with_name("TEST")
            .with_bind_point(AclBindPointType::Port)
            .with_match(AclMatchField::SrcIp)
            .with_action(AclActionType::PacketAction)
            .build()
            .unwrap();

        assert_eq!(tt.name, "TEST");
        assert!(tt.supports_match(AclMatchField::SrcIp));
        assert!(!tt.supports_match(AclMatchField::DstIp));
        assert!(tt.supports_action(AclActionType::PacketAction));
        assert!(tt.supports_bind_point(AclBindPointType::Port));
    }

    #[test]
    fn test_table_type_builder_validation() {
        // Missing name
        let result = AclTableTypeBuilder::new()
            .with_bind_point(AclBindPointType::Port)
            .with_match(AclMatchField::SrcIp)
            .build();
        assert!(result.is_err());

        // Missing bind points
        let result = AclTableTypeBuilder::new()
            .with_name("TEST")
            .with_match(AclMatchField::SrcIp)
            .build();
        assert!(result.is_err());

        // Missing matches and actions
        let result = AclTableTypeBuilder::new()
            .with_name("TEST")
            .with_bind_point(AclBindPointType::Port)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builtin_l3() {
        let tt = create_l3_table_type();
        assert_eq!(tt.name, "L3");
        assert!(tt.is_builtin);
        assert!(tt.supports_match(AclMatchField::SrcIp));
        assert!(tt.supports_match(AclMatchField::DstIp));
        assert!(tt.supports_match(AclMatchField::IpProtocol));
        assert!(tt.supports_action(AclActionType::PacketAction));
        assert!(tt.supports_action(AclActionType::Redirect));
        assert!(tt.supports_bind_point(AclBindPointType::Port));
    }

    #[test]
    fn test_builtin_mirror() {
        let tt = create_mirror_table_type();
        assert_eq!(tt.name, "MIRROR");
        assert!(tt.supports_action(AclActionType::MirrorIngress));
        assert!(tt.supports_action(AclActionType::MirrorEgress));
    }

    #[test]
    fn test_validate_matches() {
        let tt = create_l3_table_type();

        // Valid matches
        let matches: HashSet<_> = [AclMatchField::SrcIp, AclMatchField::DstIp].into();
        assert!(tt.validate_matches(&matches).is_ok());

        // Invalid match
        let matches: HashSet<_> = [AclMatchField::SrcIp, AclMatchField::SrcIpv6].into();
        assert!(tt.validate_matches(&matches).is_err());
    }

    #[test]
    fn test_validate_actions() {
        let tt = create_l3_table_type();

        // Valid actions
        let actions: HashSet<_> = [AclActionType::PacketAction].into();
        assert!(tt.validate_actions(&actions).is_ok());

        // Invalid action (L3 doesn't support mirror)
        let actions: HashSet<_> = [AclActionType::MirrorIngress].into();
        assert!(tt.validate_actions(&actions).is_err());
    }

    #[test]
    fn test_stage_support() {
        // L3 supports both stages (no stages specified = both)
        let tt = create_l3_table_type();
        assert!(tt.supports_stage(AclStage::Ingress));
        assert!(tt.supports_stage(AclStage::Egress));

        // PFCWD only supports ingress
        let tt = create_pfcwd_table_type();
        assert!(tt.supports_stage(AclStage::Ingress));
        assert!(!tt.supports_stage(AclStage::Egress));
    }
}
