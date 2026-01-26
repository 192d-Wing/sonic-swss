//! Shell command builders for VLAN operations

use sonic_cfgmgr_common::shell;

/// Dot1Q bridge name
pub const DOT1Q_BRIDGE_NAME: &str = "Bridge";

/// VLAN interface prefix
pub const VLAN_PREFIX: &str = "Vlan";

/// LAG (PortChannel) prefix
pub const LAG_PREFIX: &str = "PortChannel";

/// Default VLAN ID
pub const DEFAULT_VLAN_ID: &str = "1";

/// Default MTU
pub const DEFAULT_MTU: &str = "9100";

/// Build bridge initialization command
///
/// Creates the dot1q bridge with proper configuration
pub fn build_init_bridge_cmd(mac_address: &str) -> String {
    format!(
        r#"{} -c "{} link del {} 2>/dev/null; \
           {} link add {} up type bridge && \
           {} link set {} mtu {} && \
           {} link set {} address {} && \
           {} vlan del vid {} dev {} self; \
           {} link del dev dummy 2>/dev/null; \
           {} link add dummy type dummy && \
           {} link set dummy master {} && \
           {} link set dummy up; \
           {} link set {} down && \
           {} link set {} up""#,
        shell::BASH_CMD,
        shell::IP_CMD,
        DOT1Q_BRIDGE_NAME,
        shell::IP_CMD,
        DOT1Q_BRIDGE_NAME,
        shell::IP_CMD,
        DOT1Q_BRIDGE_NAME,
        DEFAULT_MTU,
        shell::IP_CMD,
        DOT1Q_BRIDGE_NAME,
        mac_address,
        shell::BRIDGE_CMD,
        DEFAULT_VLAN_ID,
        DOT1Q_BRIDGE_NAME,
        shell::IP_CMD,
        shell::IP_CMD,
        shell::IP_CMD,
        DOT1Q_BRIDGE_NAME,
        shell::IP_CMD,
        shell::IP_CMD,
        DOT1Q_BRIDGE_NAME,
        shell::IP_CMD,
        DOT1Q_BRIDGE_NAME,
    )
}

/// Build VLAN filtering enable command
pub fn build_vlan_filtering_cmd() -> String {
    format!(
        "{} link set {} type bridge vlan_filtering 1",
        shell::IP_CMD,
        DOT1Q_BRIDGE_NAME
    )
}

/// Build no link-local learn command
pub fn build_no_linklocal_learn_cmd() -> String {
    format!(
        "{} link set {} type bridge no_linklocal_learn 1",
        shell::IP_CMD,
        DOT1Q_BRIDGE_NAME
    )
}

/// Build check if bridge exists command
pub fn build_check_bridge_exists_cmd() -> String {
    format!(
        "{} link show {} 2>/dev/null",
        shell::IP_CMD,
        DOT1Q_BRIDGE_NAME
    )
}

/// Build add VLAN command
pub fn build_add_vlan_cmd(vlan_id: u16, mac_address: &str) -> String {
    format!(
        r#"{} -c "{} vlan add vid {} dev {} self && \
           {} link add link {} up name {}{} address {} type vlan id {}""#,
        shell::BASH_CMD,
        shell::BRIDGE_CMD,
        vlan_id,
        DOT1Q_BRIDGE_NAME,
        shell::IP_CMD,
        DOT1Q_BRIDGE_NAME,
        VLAN_PREFIX,
        vlan_id,
        mac_address,
        vlan_id
    )
}

/// Build ARP evict nocarrier disable command
pub fn build_arp_evict_nocarrier_cmd(vlan_id: u16) -> String {
    format!(
        "{} 0 > /proc/sys/net/ipv4/conf/{}{}/arp_evict_nocarrier",
        shell::ECHO_CMD,
        VLAN_PREFIX,
        vlan_id
    )
}

/// Build remove VLAN command
pub fn build_remove_vlan_cmd(vlan_id: u16) -> String {
    format!(
        r#"{} -c "{} link del {}{} && \
           {} vlan del vid {} dev {} self""#,
        shell::BASH_CMD,
        shell::IP_CMD,
        VLAN_PREFIX,
        vlan_id,
        shell::BRIDGE_CMD,
        vlan_id,
        DOT1Q_BRIDGE_NAME
    )
}

/// Build set VLAN admin state command
pub fn build_set_vlan_admin_cmd(vlan_id: u16, admin_status: &str) -> String {
    let admin_quoted = shell::shellquote(admin_status);
    format!(
        "{} link set {}{} {}",
        shell::IP_CMD,
        VLAN_PREFIX,
        vlan_id,
        admin_quoted
    )
}

/// Build set VLAN MTU command
pub fn build_set_vlan_mtu_cmd(vlan_id: u16, mtu: u32) -> String {
    format!(
        "{} link set {}{} mtu {}",
        shell::IP_CMD,
        VLAN_PREFIX,
        vlan_id,
        mtu
    )
}

/// Build set VLAN MAC address command
pub fn build_set_vlan_mac_cmd(vlan_id: u16, mac: &str) -> String {
    let mac_quoted = shell::shellquote(mac);
    let bridge_down = format!("{} link set {} down", shell::IP_CMD, DOT1Q_BRIDGE_NAME);
    let set_mac = format!(
        "{} link set {}{} address {} && {} link set {} address {}",
        shell::IP_CMD,
        VLAN_PREFIX,
        vlan_id,
        mac_quoted,
        shell::IP_CMD,
        DOT1Q_BRIDGE_NAME,
        mac_quoted
    );
    let bridge_up = format!("{} link set {} up", shell::IP_CMD, DOT1Q_BRIDGE_NAME);

    format!("{} && {} && {}", bridge_down, set_mac, bridge_up)
}

/// Build add VLAN member command
pub fn build_add_vlan_member_cmd(vlan_id: u16, port_alias: &str, tagging_cmd: &str) -> String {
    let port_quoted = shell::shellquote(port_alias);
    let inner = format!(
        "{} link set {} master {} && \
         {} vlan del vid {} dev {} && \
         {} vlan add vid {} dev {} {}",
        shell::IP_CMD,
        port_quoted,
        DOT1Q_BRIDGE_NAME,
        shell::BRIDGE_CMD,
        DEFAULT_VLAN_ID,
        port_quoted,
        shell::BRIDGE_CMD,
        vlan_id,
        port_quoted,
        tagging_cmd
    );
    format!("{} -c {}", shell::BASH_CMD, shell::shellquote(&inner))
}

/// Build remove VLAN member command
///
/// This command is complex: it removes the VLAN from the port, then checks if
/// the port has any remaining VLANs. If not, it detaches the port from the bridge.
pub fn build_remove_vlan_member_cmd(vlan_id: u16, port_alias: &str) -> String {
    let port_quoted = shell::shellquote(port_alias);
    let inner = format!(
        r#"{} vlan del vid {} dev {} && \
           ( vlanShow=$({} vlan show dev {}); \
           ret=$?; \
           if [ $ret -eq 0 ]; then \
           if (! echo "$vlanShow" | {} -q {}) \
             || (echo "$vlanShow" | {} -q None$) \
             || (echo "$vlanShow" | {} -q {}$); then \
           {} link set {} nomaster; \
           fi; \
           else exit $ret; fi )"#,
        shell::BRIDGE_CMD,
        vlan_id,
        port_quoted,
        shell::BRIDGE_CMD,
        port_quoted,
        shell::GREP_CMD,
        port_quoted,
        shell::GREP_CMD,
        shell::GREP_CMD,
        port_quoted,
        shell::IP_CMD,
        port_quoted
    );
    format!("{} -c {}", shell::BASH_CMD, shell::shellquote(&inner))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_init_bridge_cmd() {
        let cmd = build_init_bridge_cmd("00:11:22:33:44:55");
        assert!(cmd.contains("ip link add Bridge"));
        assert!(cmd.contains("00:11:22:33:44:55"));
        assert!(cmd.contains("mtu 9100"));
        assert!(cmd.contains("dummy"));
    }

    #[test]
    fn test_build_vlan_filtering_cmd() {
        let cmd = build_vlan_filtering_cmd();
        assert!(cmd.contains("vlan_filtering 1"));
    }

    #[test]
    fn test_build_add_vlan_cmd() {
        let cmd = build_add_vlan_cmd(100, "00:11:22:33:44:55");
        assert!(cmd.contains("vlan add vid 100"));
        assert!(cmd.contains("Vlan100"));
        assert!(cmd.contains("00:11:22:33:44:55"));
    }

    #[test]
    fn test_build_remove_vlan_cmd() {
        let cmd = build_remove_vlan_cmd(100);
        assert!(cmd.contains("ip link del Vlan100"));
        assert!(cmd.contains("vlan del vid 100"));
    }

    #[test]
    fn test_build_set_vlan_admin_cmd() {
        let cmd = build_set_vlan_admin_cmd(100, "up");
        assert!(cmd.contains("Vlan100"));
        assert!(cmd.contains("up"));
    }

    #[test]
    fn test_build_set_vlan_mtu_cmd() {
        let cmd = build_set_vlan_mtu_cmd(100, 1500);
        assert!(cmd.contains("Vlan100"));
        assert!(cmd.contains("mtu 1500"));
    }

    #[test]
    fn test_build_add_vlan_member_cmd() {
        let cmd = build_add_vlan_member_cmd(100, "Ethernet0", "pvid untagged");
        assert!(cmd.contains("Ethernet0"));
        assert!(cmd.contains("vid 100"));
        assert!(cmd.contains("pvid untagged"));
    }

    #[test]
    fn test_build_remove_vlan_member_cmd() {
        let cmd = build_remove_vlan_member_cmd(100, "Ethernet0");
        assert!(cmd.contains("vlan del vid 100"));
        assert!(cmd.contains("Ethernet0"));
        assert!(cmd.contains("nomaster"));
    }

    #[test]
    fn test_shellquote_safety() {
        // Test that dangerous characters are properly quoted
        let cmd = build_add_vlan_member_cmd(100, "Ethernet0; rm -rf /", "");
        // The inner command gets shellquoted, which means the quotes around the port
        // name get escaped. This prevents command injection.
        assert!(cmd.contains("\\\"Ethernet0; rm -rf /\\\""));
    }
}
