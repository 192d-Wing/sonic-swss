//! Configuration merging logic

use crate::types::CoppCfg;
use sonic_cfgmgr_common::{CfgMgrResult, FieldValues, FieldValuesExt};
use tracing::{debug, info};

/// Merge init config with user CONFIG_DB config
///
/// Rules:
/// 1. User fields override init fields
/// 2. If user config has "NULL" field, suppress the entire init entry
/// 3. User-only entries (not in init) are included as-is
///
/// # Arguments
/// * `init_cfg` - Configuration from JSON init file
/// * `cfg_keys` - Keys present in CONFIG_DB
/// * `user_cfg_getter` - Function to fetch user config for a key
///
/// # Returns
/// Merged configuration
pub fn merge_config<F>(
    init_cfg: &CoppCfg,
    cfg_keys: &[String],
    mut user_cfg_getter: F,
) -> CfgMgrResult<CoppCfg>
where
    F: FnMut(&str) -> CfgMgrResult<FieldValues>,
{
    let mut merged = CoppCfg::new();

    // Process init entries
    for (key, init_fvs) in init_cfg {
        if cfg_keys.contains(key) {
            let user_fvs = user_cfg_getter(key)?;

            // Check for NULL suppression
            if user_fvs.has_field("NULL") {
                debug!("Ignoring create for key {} due to NULL field", key);
                continue; // Skip this entry entirely
            }

            // Merge: user fields override init fields
            let mut merged_fvs = user_fvs.clone();

            // Add init fields not present in user config
            for (init_field, init_value) in init_fvs {
                if !user_fvs.iter().any(|(f, _)| f == init_field) {
                    merged_fvs.push((init_field.clone(), init_value.clone()));
                }
            }

            info!(
                "Merged config for {}: {} user fields + {} init fields = {} total",
                key,
                user_fvs.len(),
                init_fvs.len(),
                merged_fvs.len()
            );
            merged.insert(key.clone(), merged_fvs);
        } else {
            // No user config, use init config as-is
            debug!("Using init config for {} (no user override)", key);
            merged.insert(key.clone(), init_fvs.clone());
        }
    }

    // Add user-only entries (not in init config)
    for key in cfg_keys {
        if !init_cfg.contains_key(key) {
            let user_fvs = user_cfg_getter(key)?;
            info!("Adding user-only config for {}", key);
            merged.insert(key.clone(), user_fvs);
        }
    }

    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fvs(items: &[(&str, &str)]) -> FieldValues {
        items
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_merge_config_user_override() {
        let mut init_cfg = CoppCfg::new();
        init_cfg.insert(
            "queue1_group1".to_string(),
            make_fvs(&[("queue", "1"), ("cir", "600"), ("cbs", "600")]),
        );

        let cfg_keys = vec!["queue1_group1".to_string()];

        let user_cfg_getter = |_key: &str| Ok(make_fvs(&[("cir", "1000")]));

        let merged = merge_config(&init_cfg, &cfg_keys, user_cfg_getter).unwrap();

        assert_eq!(merged.len(), 1);
        let fvs = &merged["queue1_group1"];

        // User override: cir=1000
        assert_eq!(
            fvs.iter()
                .find(|(k, _)| k == "cir")
                .map(|(_, v)| v.as_str()),
            Some("1000")
        );

        // Init values preserved: queue=1, cbs=600
        assert_eq!(
            fvs.iter()
                .find(|(k, _)| k == "queue")
                .map(|(_, v)| v.as_str()),
            Some("1")
        );
        assert_eq!(
            fvs.iter()
                .find(|(k, _)| k == "cbs")
                .map(|(_, v)| v.as_str()),
            Some("600")
        );
    }

    #[test]
    fn test_merge_config_null_suppression() {
        let mut init_cfg = CoppCfg::new();
        init_cfg.insert("arp".to_string(), make_fvs(&[("trap_ids", "arp_req")]));

        let cfg_keys = vec!["arp".to_string()];

        let user_cfg_getter = |_key: &str| Ok(make_fvs(&[("NULL", "")]));

        let merged = merge_config(&init_cfg, &cfg_keys, user_cfg_getter).unwrap();

        // Entry should be suppressed
        assert!(!merged.contains_key("arp"));
    }

    #[test]
    fn test_merge_config_user_only() {
        let init_cfg = CoppCfg::new(); // Empty init

        let cfg_keys = vec!["custom_trap".to_string()];

        let user_cfg_getter = |_key: &str| Ok(make_fvs(&[("trap_ids", "custom_id")]));

        let merged = merge_config(&init_cfg, &cfg_keys, user_cfg_getter).unwrap();

        assert_eq!(merged.len(), 1);
        assert!(merged.contains_key("custom_trap"));
        let fvs = &merged["custom_trap"];
        assert_eq!(
            fvs.iter()
                .find(|(k, _)| k == "trap_ids")
                .map(|(_, v)| v.as_str()),
            Some("custom_id")
        );
    }

    #[test]
    fn test_merge_config_init_only() {
        let mut init_cfg = CoppCfg::new();
        init_cfg.insert("bgp".to_string(), make_fvs(&[("trap_ids", "bgp,bgpv6")]));

        let cfg_keys = vec![]; // No user config

        let user_cfg_getter = |_key: &str| {
            panic!("Should not be called");
        };

        let merged = merge_config(&init_cfg, &cfg_keys, user_cfg_getter).unwrap();

        assert_eq!(merged.len(), 1);
        assert!(merged.contains_key("bgp"));
        let fvs = &merged["bgp"];
        assert_eq!(
            fvs.iter()
                .find(|(k, _)| k == "trap_ids")
                .map(|(_, v)| v.as_str()),
            Some("bgp,bgpv6")
        );
    }

    #[test]
    fn test_merge_config_multiple_entries() {
        let mut init_cfg = CoppCfg::new();
        init_cfg.insert("arp".to_string(), make_fvs(&[("trap_ids", "arp_req")]));
        init_cfg.insert("bgp".to_string(), make_fvs(&[("trap_ids", "bgp")]));

        let cfg_keys = vec!["arp".to_string(), "custom".to_string()];

        let user_cfg_getter = |key: &str| match key {
            "arp" => Ok(make_fvs(&[("trap_group", "queue1")])),
            "custom" => Ok(make_fvs(&[("trap_ids", "custom_id")])),
            _ => panic!("Unexpected key: {}", key),
        };

        let merged = merge_config(&init_cfg, &cfg_keys, user_cfg_getter).unwrap();

        assert_eq!(merged.len(), 3); // arp (merged), bgp (init only), custom (user only)

        // arp: merged
        let arp_fvs = &merged["arp"];
        assert!(arp_fvs
            .iter()
            .any(|(k, v)| k == "trap_ids" && v == "arp_req"));
        assert!(arp_fvs
            .iter()
            .any(|(k, v)| k == "trap_group" && v == "queue1"));

        // bgp: init only
        let bgp_fvs = &merged["bgp"];
        assert!(bgp_fvs.iter().any(|(k, v)| k == "trap_ids" && v == "bgp"));

        // custom: user only
        let custom_fvs = &merged["custom"];
        assert!(custom_fvs
            .iter()
            .any(|(k, v)| k == "trap_ids" && v == "custom_id"));
    }
}
