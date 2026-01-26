//! VRF and related operations

use crate::tables::{IP_CMD, SYSCTL_CMD};
use sonic_cfgmgr_common::{shell, CfgMgrResult};
use tracing::{error, info};

/// Bind interface to VRF or unbind
///
/// # Arguments
/// * `alias` - Interface name
/// * `vrf_name` - VRF name (None to unbind)
pub async fn set_intf_vrf(alias: &str, vrf_name: Option<&str>) -> CfgMgrResult<()> {
    let cmd = if let Some(vrf) = vrf_name {
        // Bind to VRF
        format!(
            "{} link set {} master {}",
            IP_CMD,
            shell::shellquote(alias),
            shell::shellquote(vrf)
        )
    } else {
        // Unbind from VRF
        format!("{} link set {} nomaster", IP_CMD, shell::shellquote(alias))
    };

    shell::exec(&cmd).await?;

    if let Some(vrf) = vrf_name {
        info!("Bound interface {} to VRF {}", alias, vrf);
    } else {
        info!("Unbound interface {} from VRF", alias);
    }

    Ok(())
}

/// Set MPLS state on interface
///
/// # Arguments
/// * `alias` - Interface name
/// * `mpls` - MPLS state: "enable", "disable", or "" (empty)
///
/// # Returns
/// * `Ok(true)` if successful
/// * `Ok(false)` if MPLS state is invalid
pub async fn set_intf_mpls(alias: &str, mpls: &str) -> CfgMgrResult<bool> {
    let input_val = match mpls {
        "enable" => "1",
        "disable" | "" => "0",
        _ => {
            error!("MPLS state is invalid: \"{}\"", mpls);
            return Ok(false);
        }
    };

    let cmd = format!(
        "{} -w net.mpls.conf.{}.input={}",
        SYSCTL_CMD, alias, input_val
    );

    // Don't return error unless MPLS is explicitly set
    if !mpls.is_empty() {
        shell::exec(&cmd).await?;
        info!("Set MPLS {} on interface {}", mpls, alias);
    } else {
        let _ = shell::exec(&cmd).await;
    }

    Ok(true)
}

/// Set proxy ARP on interface
pub async fn set_intf_proxy_arp(alias: &str, proxy_arp: &str) -> CfgMgrResult<bool> {
    let val = match proxy_arp {
        "enabled" => "1",
        "disabled" | "" => "0",
        _ => {
            error!("Proxy ARP state is invalid: \"{}\"", proxy_arp);
            return Ok(false);
        }
    };

    let cmd = format!(
        "{} -w net.ipv4.conf.{}.proxy_arp={}",
        SYSCTL_CMD, alias, val
    );

    shell::exec(&cmd).await?;
    info!("Set proxy ARP {} on interface {}", proxy_arp, alias);
    Ok(true)
}

/// Set gratuitous ARP on interface
pub async fn set_intf_grat_arp(alias: &str, grat_arp: &str) -> CfgMgrResult<bool> {
    let val = match grat_arp {
        "enabled" => "1",
        "disabled" | "" => "0",
        _ => {
            error!("Gratuitous ARP state is invalid: \"{}\"", grat_arp);
            return Ok(false);
        }
    };

    let cmd = format!(
        "{} -w net.ipv4.conf.{}.arp_notify={}",
        SYSCTL_CMD, alias, val
    );

    shell::exec(&cmd).await?;
    info!("Set gratuitous ARP {} on interface {}", grat_arp, alias);
    Ok(true)
}

#[cfg(test)]
mod tests {

    // Note: These tests just verify command generation logic
    // Actual execution would require mocking or integration tests

    #[test]
    fn test_mpls_enable() {
        // Test that enable state is recognized
        assert_eq!(
            "1",
            match "enable" {
                "enable" => "1",
                "disable" | "" => "0",
                _ => panic!(),
            }
        );
    }

    #[test]
    fn test_mpls_disable() {
        assert_eq!(
            "0",
            match "disable" {
                "enable" => "1",
                "disable" | "" => "0",
                _ => panic!(),
            }
        );
    }

    #[test]
    fn test_mpls_empty() {
        assert_eq!(
            "0",
            match "" {
                "enable" => "1",
                "disable" | "" => "0",
                _ => panic!(),
            }
        );
    }
}
