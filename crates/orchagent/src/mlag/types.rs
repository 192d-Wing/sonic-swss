//! MLAG types and data structures.

/// MLAG interface update notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MlagIfUpdate {
    /// Interface name.
    pub if_name: String,
    /// True for add, false for delete.
    pub is_add: bool,
}

impl MlagIfUpdate {
    /// Creates an add notification.
    pub fn add(if_name: impl Into<String>) -> Self {
        Self {
            if_name: if_name.into(),
            is_add: true,
        }
    }

    /// Creates a delete notification.
    pub fn delete(if_name: impl Into<String>) -> Self {
        Self {
            if_name: if_name.into(),
            is_add: false,
        }
    }
}

/// MLAG ISL (Inter-Switch Link) update notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MlagIslUpdate {
    /// ISL interface name.
    pub isl_name: String,
    /// True for add, false for delete.
    pub is_add: bool,
}

impl MlagIslUpdate {
    /// Creates an add notification.
    pub fn add(isl_name: impl Into<String>) -> Self {
        Self {
            isl_name: isl_name.into(),
            is_add: true,
        }
    }

    /// Creates a delete notification.
    pub fn delete(isl_name: impl Into<String>) -> Self {
        Self {
            isl_name: isl_name.into(),
            is_add: false,
        }
    }
}

/// MLAG subject types for observer notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MlagSubjectType {
    /// ISL (peer-link) changed.
    IslChange,
    /// MLAG interface membership changed.
    IntfChange,
}

/// MLAG update type combining both ISL and interface updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MlagUpdate {
    /// ISL change notification.
    Isl(MlagIslUpdate),
    /// Interface change notification.
    Intf(MlagIfUpdate),
}

impl MlagUpdate {
    /// Returns the subject type for this update.
    pub fn subject_type(&self) -> MlagSubjectType {
        match self {
            Self::Isl(_) => MlagSubjectType::IslChange,
            Self::Intf(_) => MlagSubjectType::IntfChange,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mlag_if_update() {
        let add = MlagIfUpdate::add("Ethernet0");
        assert_eq!(add.if_name, "Ethernet0");
        assert!(add.is_add);

        let del = MlagIfUpdate::delete("Ethernet0");
        assert_eq!(del.if_name, "Ethernet0");
        assert!(!del.is_add);
    }

    #[test]
    fn test_mlag_isl_update() {
        let add = MlagIslUpdate::add("PortChannel100");
        assert_eq!(add.isl_name, "PortChannel100");
        assert!(add.is_add);

        let del = MlagIslUpdate::delete("PortChannel100");
        assert_eq!(del.isl_name, "PortChannel100");
        assert!(!del.is_add);
    }

    #[test]
    fn test_mlag_update() {
        let isl = MlagUpdate::Isl(MlagIslUpdate::add("PortChannel100"));
        assert_eq!(isl.subject_type(), MlagSubjectType::IslChange);

        let intf = MlagUpdate::Intf(MlagIfUpdate::add("Ethernet0"));
        assert_eq!(intf.subject_type(), MlagSubjectType::IntfChange);
    }
}
