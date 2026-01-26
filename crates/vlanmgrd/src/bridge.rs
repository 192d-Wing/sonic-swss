//! Bridge initialization for vlanmgrd

use crate::commands::{
    build_check_bridge_exists_cmd, build_init_bridge_cmd, build_no_linklocal_learn_cmd,
    build_vlan_filtering_cmd,
};
use sonic_cfgmgr_common::{shell, CfgMgrResult};
use tracing::{debug, info};

/// Initialize the dot1q bridge
///
/// Creates the Bridge interface with VLAN filtering enabled.
/// This is called on daemon startup unless warm restart is active
/// and the bridge already exists.
pub async fn init_bridge(mac_address: &str, mock_mode: bool) -> CfgMgrResult<()> {
    info!("Initializing dot1q bridge");

    if mock_mode {
        debug!("Mock mode: skipping actual bridge initialization");
        return Ok(());
    }

    // Create bridge with dummy interface
    let init_cmd = build_init_bridge_cmd(mac_address);
    shell::exec(&init_cmd).await?;
    info!("Bridge created successfully");

    // Enable VLAN filtering
    let vlan_filter_cmd = build_vlan_filtering_cmd();
    shell::exec(&vlan_filter_cmd).await?;
    info!("VLAN filtering enabled");

    // Disable link-local learning
    let no_ll_cmd = build_no_linklocal_learn_cmd();
    shell::exec(&no_ll_cmd).await?;
    info!("Link-local learning disabled");

    Ok(())
}

/// Check if bridge already exists
///
/// Used during warm restart to determine if bridge initialization
/// should be skipped.
pub async fn bridge_exists(mock_mode: bool) -> CfgMgrResult<bool> {
    if mock_mode {
        return Ok(false);
    }

    let check_cmd = build_check_bridge_exists_cmd();
    match shell::exec(&check_cmd).await {
        Ok(_) => {
            info!("Bridge already exists");
            Ok(true)
        }
        Err(_) => {
            debug!("Bridge does not exist");
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_init_bridge_mock_mode() {
        // In mock mode, should succeed without executing commands
        let result = init_bridge("00:11:22:33:44:55", true).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_bridge_exists_mock_mode() {
        // In mock mode, should always return false
        let result = bridge_exists(true).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
