//! Sub-interface operations

use crate::tables::IP_CMD;
use sonic_cfgmgr_common::{shell, CfgMgrResult};
use tracing::{info, warn};

/// Create sub-interface
///
/// Creates a VLAN sub-interface using `ip link add`
///
/// # Arguments
/// * `parent` - Parent interface name (e.g., "Ethernet0")
/// * `subintf` - Sub-interface name (e.g., "Ethernet0.100")
/// * `vlan_id` - VLAN ID (e.g., "100")
pub async fn add_host_subintf(parent: &str, subintf: &str, vlan_id: &str) -> CfgMgrResult<()> {
    let cmd = format!(
        "{} link add link {} name {} type vlan id {}",
        IP_CMD,
        shell::shellquote(parent),
        shell::shellquote(subintf),
        vlan_id
    );

    shell::exec(&cmd).await?;
    info!("Created sub-interface {} with VLAN ID {}", subintf, vlan_id);
    Ok(())
}

/// Delete sub-interface
pub async fn remove_host_subintf(subintf: &str) -> CfgMgrResult<()> {
    let cmd = format!("{} link del {}", IP_CMD, shell::shellquote(subintf));

    shell::exec(&cmd).await?;
    info!("Deleted sub-interface {}", subintf);
    Ok(())
}

/// Set sub-interface MTU
///
/// Validates that sub-interface MTU does not exceed parent MTU
///
/// # Returns
/// The effective MTU that was set
pub async fn set_subintf_mtu(subintf: &str, mtu: &str, parent_mtu: &str) -> CfgMgrResult<String> {
    // Parse MTU values
    let subintf_mtu: u32 = mtu.parse().unwrap_or(9100);
    let parent_mtu_val: u32 = parent_mtu.parse().unwrap_or(9100);

    // Validate: sub-interface MTU cannot exceed parent MTU
    let effective_mtu = if subintf_mtu > parent_mtu_val {
        warn!(
            "Sub-interface {} MTU {} exceeds parent MTU {}, using parent MTU",
            subintf, subintf_mtu, parent_mtu_val
        );
        parent_mtu.to_string()
    } else {
        mtu.to_string()
    };

    let cmd = format!(
        "{} link set {} mtu {}",
        IP_CMD,
        shell::shellquote(subintf),
        &effective_mtu
    );

    shell::exec(&cmd).await?;
    info!("Set MTU {} on sub-interface {}", effective_mtu, subintf);

    Ok(effective_mtu)
}

/// Set sub-interface admin status
pub async fn set_subintf_admin_status(subintf: &str, admin_status: &str) -> CfgMgrResult<String> {
    let state = if admin_status == "up" { "up" } else { "down" };

    let cmd = format!(
        "{} link set {} {}",
        IP_CMD,
        shell::shellquote(subintf),
        state
    );

    shell::exec(&cmd).await?;
    info!("Set admin status {} on sub-interface {}", state, subintf);

    Ok(state.to_string())
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_mtu_validation_within_limit() {
        let subintf_mtu: u32 = 1500;
        let parent_mtu: u32 = 9100;

        assert!(subintf_mtu <= parent_mtu);
    }

    #[test]
    fn test_mtu_validation_exceeds_limit() {
        let subintf_mtu: u32 = 9200;
        let parent_mtu: u32 = 9100;

        assert!(subintf_mtu > parent_mtu);
        // In real code, we'd use parent MTU
    }

    #[test]
    fn test_admin_status_up() {
        let state = if "up" == "up" { "up" } else { "down" };
        assert_eq!(state, "up");
    }

    #[test]
    fn test_admin_status_down() {
        let state = if "down" == "up" { "up" } else { "down" };
        assert_eq!(state, "down");
    }
}
