//! Shell command builders for VRF operations

use sonic_cfgmgr_common::shell;

use crate::types::TABLE_LOCAL_PREF;

/// Build VRF creation command
///
/// Creates a VRF device with the specified routing table ID
pub fn build_add_vrf_cmd(vrf_name: &str, table_id: u32) -> String {
    format!(
        "{} link add {} type vrf table {}",
        shell::IP_CMD,
        shell::shellquote(vrf_name),
        table_id
    )
}

/// Build VRF bring-up command
///
/// Brings up the VRF device
pub fn build_set_vrf_up_cmd(vrf_name: &str) -> String {
    format!(
        "{} link set {} up",
        shell::IP_CMD,
        shell::shellquote(vrf_name)
    )
}

/// Build VRF deletion command
///
/// Deletes the VRF device
pub fn build_del_vrf_cmd(vrf_name: &str) -> String {
    format!("{} link del {}", shell::IP_CMD, shell::shellquote(vrf_name))
}

/// Build query existing VRFs command
///
/// Lists all existing VRF devices with details
pub fn build_show_vrf_cmd() -> String {
    format!("{} -d link show type vrf", shell::IP_CMD)
}

/// Build local routing rule setup commands
///
/// Sets up priority 1001 local routing rules for IPv4 and IPv6
pub fn build_local_routing_rules_cmd() -> String {
    format!(
        "{} rule add pref {} table local && {} rule del pref 0 && \
         {} -6 rule add pref {} table local && {} -6 rule del pref 0",
        shell::IP_CMD,
        TABLE_LOCAL_PREF,
        shell::IP_CMD,
        shell::IP_CMD,
        TABLE_LOCAL_PREF,
        shell::IP_CMD
    )
}

/// Build check for priority 0 rule command
///
/// Checks if priority 0 rule exists
pub fn build_check_prio0_rule_cmd() -> String {
    format!("{} rule | {} '^0:'", shell::IP_CMD, shell::GREP_CMD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_add_vrf_cmd() {
        let cmd = build_add_vrf_cmd("Vrf1", 1001);
        assert!(cmd.contains("ip link add"));
        assert!(cmd.contains("Vrf1"));
        assert!(cmd.contains("type vrf table 1001"));
    }

    #[test]
    fn test_build_set_vrf_up_cmd() {
        let cmd = build_set_vrf_up_cmd("Vrf1");
        assert!(cmd.contains("ip link set"));
        assert!(cmd.contains("Vrf1"));
        assert!(cmd.contains("up"));
    }

    #[test]
    fn test_build_del_vrf_cmd() {
        let cmd = build_del_vrf_cmd("Vrf1");
        assert!(cmd.contains("ip link del"));
        assert!(cmd.contains("Vrf1"));
    }

    #[test]
    fn test_build_show_vrf_cmd() {
        let cmd = build_show_vrf_cmd();
        assert!(cmd.contains("ip -d link show type vrf"));
    }

    #[test]
    fn test_build_local_routing_rules_cmd() {
        let cmd = build_local_routing_rules_cmd();
        assert!(cmd.contains("ip rule add pref 1001 table local"));
        assert!(cmd.contains("ip rule del pref 0"));
        assert!(cmd.contains("ip -6 rule add pref 1001 table local"));
        assert!(cmd.contains("ip -6 rule del pref 0"));
    }

    #[test]
    fn test_shellquote_safety() {
        let cmd = build_add_vrf_cmd("Vrf'; rm -rf /", 1001);
        // Should be quoted to prevent injection
        assert!(cmd.contains("\"Vrf'; rm -rf /\""));
    }
}
