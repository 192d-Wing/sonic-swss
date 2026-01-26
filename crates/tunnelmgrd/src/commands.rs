//! Shell command builders for tunnel operations

use sonic_cfgmgr_common::shell;

use crate::types::{IpPrefix, TunnelInfo, TUNNEL_INTERFACE};

/// Build tunnel interface creation command
///
/// Creates an IP-in-IP tunnel with local and remote endpoints
pub fn build_add_tunnel_cmd(info: &TunnelInfo) -> String {
    format!(
        "{} tunnel add {} mode ipip local {} remote {}",
        shell::IP_CMD,
        TUNNEL_INTERFACE,
        shell::shellquote(&info.dst_ip),
        shell::shellquote(&info.remote_ip)
    )
}

/// Build tunnel interface deletion command
pub fn build_del_tunnel_cmd() -> String {
    format!("{} tunnel del {}", shell::IP_CMD, TUNNEL_INTERFACE)
}

/// Build tunnel interface bring-up command
pub fn build_set_tunnel_up_cmd() -> String {
    format!("{} link set dev {} up", shell::IP_CMD, TUNNEL_INTERFACE)
}

/// Build tunnel address assignment command
///
/// Assigns an IP address to the tunnel interface
pub fn build_add_tunnel_address_cmd(ip_prefix: &str) -> String {
    format!(
        "{} addr add {} dev {}",
        shell::IP_CMD,
        shell::shellquote(ip_prefix),
        TUNNEL_INTERFACE
    )
}

/// Build tunnel route add/replace command
///
/// Routes traffic to a prefix through the tunnel interface.
/// Uses 'replace' to handle existing routes gracefully.
pub fn build_add_tunnel_route_cmd(prefix: &IpPrefix) -> String {
    if prefix.is_v4() {
        format!(
            "{} route replace {} dev {}",
            shell::IP_CMD,
            shell::shellquote(&prefix.to_string()),
            TUNNEL_INTERFACE
        )
    } else {
        format!(
            "{} -6 route replace {} dev {}",
            shell::IP_CMD,
            shell::shellquote(&prefix.to_string()),
            TUNNEL_INTERFACE
        )
    }
}

/// Build tunnel route deletion command
pub fn build_del_tunnel_route_cmd(prefix: &IpPrefix) -> String {
    if prefix.is_v4() {
        format!(
            "{} route del {} dev {}",
            shell::IP_CMD,
            shell::shellquote(&prefix.to_string()),
            TUNNEL_INTERFACE
        )
    } else {
        format!(
            "{} -6 route del {} dev {}",
            shell::IP_CMD,
            shell::shellquote(&prefix.to_string()),
            TUNNEL_INTERFACE
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_add_tunnel_cmd() {
        let info = TunnelInfo::new("IPINIP".to_string(), "10.1.0.32".to_string())
            .with_remote_ip("10.1.0.33".to_string());

        let cmd = build_add_tunnel_cmd(&info);
        assert!(cmd.contains("ip tunnel add tun0 mode ipip"));
        assert!(cmd.contains("local"));
        assert!(cmd.contains("remote"));
    }

    #[test]
    fn test_build_del_tunnel_cmd() {
        let cmd = build_del_tunnel_cmd();
        assert!(cmd.contains("ip tunnel del tun0"));
    }

    #[test]
    fn test_build_set_tunnel_up_cmd() {
        let cmd = build_set_tunnel_up_cmd();
        assert!(cmd.contains("ip link set dev tun0 up"));
    }

    #[test]
    fn test_build_add_tunnel_address_cmd() {
        let cmd = build_add_tunnel_address_cmd("10.0.0.1/32");
        assert!(cmd.contains("ip addr add"));
        assert!(cmd.contains("\"10.0.0.1/32\""));
        assert!(cmd.contains("dev tun0"));
    }

    #[test]
    fn test_build_tunnel_route_ipv4() {
        let prefix = "192.168.1.0/24".parse::<IpPrefix>().unwrap();
        let cmd = build_add_tunnel_route_cmd(&prefix);
        assert!(cmd.contains("ip route replace"));
        assert!(cmd.contains("\"192.168.1.0/24\""));
        assert!(!cmd.contains("-6"));
    }

    #[test]
    fn test_build_tunnel_route_ipv6() {
        let prefix = "2001:db8::/32".parse::<IpPrefix>().unwrap();
        let cmd = build_add_tunnel_route_cmd(&prefix);
        assert!(cmd.contains("ip -6 route replace"));
        assert!(cmd.contains("\"2001:db8::/32\""));
    }

    #[test]
    fn test_build_del_tunnel_route_ipv4() {
        let prefix = "192.168.1.0/24".parse::<IpPrefix>().unwrap();
        let cmd = build_del_tunnel_route_cmd(&prefix);
        assert!(cmd.contains("ip route del"));
        assert!(cmd.contains("\"192.168.1.0/24\""));
    }

    #[test]
    fn test_build_del_tunnel_route_ipv6() {
        let prefix = "2001:db8::/32".parse::<IpPrefix>().unwrap();
        let cmd = build_del_tunnel_route_cmd(&prefix);
        assert!(cmd.contains("ip -6 route del"));
        assert!(cmd.contains("\"2001:db8::/32\""));
    }

    #[test]
    fn test_shellquote_safety() {
        let info = TunnelInfo::new("IPINIP".to_string(), "10.1.0.32; rm -rf /".to_string())
            .with_remote_ip("10.1.0.33".to_string());

        let cmd = build_add_tunnel_cmd(&info);
        // Should be quoted to prevent injection
        assert!(cmd.contains("\"10.1.0.32; rm -rf /\""));
    }
}
