//! IP address operations

use crate::tables::{IP_CMD, SYSCTL_CMD};
use crate::types::SwitchType;
use sonic_cfgmgr_common::{shell, CfgMgrResult};
use sonic_types::IpPrefix;
use tracing::info;

/// Set interface IP address
pub async fn set_intf_ip(
    alias: &str,
    op: &str,
    ip_prefix: &IpPrefix,
    switch_type: &SwitchType,
) -> CfgMgrResult<()> {
    let ip_prefix_str = ip_prefix.to_string();
    let _prefix_len = ip_prefix.prefix_len();

    let cmd = if ip_prefix.is_ipv4() {
        // IPv4 address - simplified without broadcast calculation
        format!(
            "{} address {} {} dev {}",
            IP_CMD,
            op,
            shell::shellquote(&ip_prefix_str),
            shell::shellquote(alias)
        )
    } else {
        // IPv6 address
        let metric = if switch_type.is_voq() {
            " metric 256"
        } else {
            ""
        };
        format!(
            "{} -6 address {} {} dev {}{}",
            IP_CMD,
            op,
            shell::shellquote(&ip_prefix_str),
            shell::shellquote(alias),
            metric
        )
    };

    // Execute command
    let result = shell::exec(&cmd).await;

    // IPv6 retry logic
    if result.is_err() && !ip_prefix.is_ipv4() && op == "add" {
        info!("Failed to assign IPv6, enabling IPv6 and retrying");
        enable_ipv6_flag(alias).await?;
        shell::exec(&cmd).await?;
    } else if let Err(e) = result {
        return Err(e);
    }

    Ok(())
}

/// Enable IPv6 on interface
pub async fn enable_ipv6_flag(alias: &str) -> CfgMgrResult<()> {
    let cmd = format!("{} -w net.ipv6.conf.{}.disable_ipv6=0", SYSCTL_CMD, alias);
    shell::exec(&cmd).await?;
    info!("Enabled IPv6 on interface {}", alias);
    Ok(())
}

/// Set interface MAC address
pub async fn set_intf_mac(alias: &str, mac_str: &str) -> CfgMgrResult<()> {
    let cmd = format!(
        "{} link set {} address {}",
        IP_CMD,
        shell::shellquote(alias),
        shell::shellquote(mac_str)
    );

    shell::exec(&cmd).await?;
    info!("Set MAC address {} on interface {}", mac_str, alias);
    Ok(())
}
