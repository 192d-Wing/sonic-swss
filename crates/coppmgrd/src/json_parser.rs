//! JSON init file parser for CoPP configuration

use crate::tables::{CFG_COPP_GROUP_TABLE, CFG_COPP_TRAP_TABLE};
use crate::types::CoppCfg;
use serde_json::Value;
use sonic_cfgmgr_common::{CfgMgrError, CfgMgrResult, FieldValues};
use std::fs::File;
use std::io::BufReader;
use tracing::{info, warn};

/// Parse CoPP init JSON file
///
/// File format:
/// ```json
/// {
///   "COPP_TRAP": {
///     "arp": {
///       "trap_ids": "arp_req,arp_resp",
///       "trap_group": "queue1_group1",
///       "always_enabled": "true"
///     }
///   },
///   "COPP_GROUP": {
///     "queue1_group1": {
///       "queue": "1",
///       "trap_action": "trap",
///       "cir": "600"
///     }
///   }
/// }
/// ```
///
/// Returns (trap_cfg, group_cfg) tuple
pub fn parse_copp_init_file(path: &str) -> CfgMgrResult<(CoppCfg, CoppCfg)> {
    let file = File::open(path).map_err(|e| {
        CfgMgrError::internal(format!("Failed to open CoPP init file {}: {}", path, e))
    })?;

    let reader = BufReader::new(file);
    let json: Value = serde_json::from_reader(reader)
        .map_err(|e| CfgMgrError::internal(format!("Failed to parse JSON from {}: {}", path, e)))?;

    let mut trap_cfg = CoppCfg::new();
    let mut group_cfg = CoppCfg::new();

    if let Value::Object(tables) = json {
        for (table_name, table_obj) in tables {
            if let Value::Object(keys) = table_obj {
                for (key, fields_obj) in keys {
                    match parse_field_values(&fields_obj) {
                        Ok(fvs) => {
                            match table_name.as_str() {
                                CFG_COPP_TRAP_TABLE => {
                                    trap_cfg.insert(key.clone(), fvs);
                                    info!("Loaded COPP_TRAP init config for {}", key);
                                }
                                CFG_COPP_GROUP_TABLE => {
                                    group_cfg.insert(key.clone(), fvs);
                                    info!("Loaded COPP_GROUP init config for {}", key);
                                }
                                _ => {
                                    warn!("Unknown table {} in CoPP init file", table_name);
                                }
                            };
                        }
                        Err(e) => {
                            warn!("Failed to parse fields for {}: {}", key, e);
                        }
                    }
                }
            } else {
                warn!("Table {} is not an object", table_name);
            }
        }
    } else {
        return Err(CfgMgrError::internal(format!(
            "CoPP init file {} does not contain a JSON object",
            path
        )));
    }

    info!(
        "Parsed CoPP init file: {} trap entries, {} group entries",
        trap_cfg.len(),
        group_cfg.len()
    );

    Ok((trap_cfg, group_cfg))
}

/// Parse field values from JSON object
fn parse_field_values(obj: &Value) -> CfgMgrResult<FieldValues> {
    let mut fvs = FieldValues::new();

    if let Value::Object(fields) = obj {
        for (field, value) in fields {
            match value {
                Value::String(val_str) => {
                    fvs.push((field.clone(), val_str.clone()));
                }
                Value::Number(num) => {
                    fvs.push((field.clone(), num.to_string()));
                }
                Value::Bool(b) => {
                    fvs.push((field.clone(), b.to_string()));
                }
                _ => {
                    warn!("Unsupported value type for field {}: {:?}", field, value);
                }
            }
        }
    } else {
        return Err(CfgMgrError::internal("Expected JSON object for fields"));
    }

    Ok(fvs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_copp_init_file_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{
  "COPP_TRAP": {{
    "arp": {{
      "trap_ids": "arp_req,arp_resp",
      "trap_group": "queue1_group1",
      "always_enabled": "true"
    }},
    "bgp": {{
      "trap_ids": "bgp,bgpv6",
      "trap_group": "queue4_group4"
    }}
  }},
  "COPP_GROUP": {{
    "queue1_group1": {{
      "queue": "1",
      "trap_action": "trap",
      "cir": "600",
      "cbs": "600"
    }}
  }}
}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let (trap_cfg, group_cfg) = parse_copp_init_file(file.path().to_str().unwrap()).unwrap();

        // Verify trap config
        assert_eq!(trap_cfg.len(), 2);
        assert!(trap_cfg.contains_key("arp"));
        assert!(trap_cfg.contains_key("bgp"));

        let arp_fvs = &trap_cfg["arp"];
        assert_eq!(
            arp_fvs
                .iter()
                .find(|(k, _)| k == "trap_ids")
                .map(|(_, v)| v.as_str()),
            Some("arp_req,arp_resp")
        );
        assert_eq!(
            arp_fvs
                .iter()
                .find(|(k, _)| k == "trap_group")
                .map(|(_, v)| v.as_str()),
            Some("queue1_group1")
        );
        assert_eq!(
            arp_fvs
                .iter()
                .find(|(k, _)| k == "always_enabled")
                .map(|(_, v)| v.as_str()),
            Some("true")
        );

        // Verify group config
        assert_eq!(group_cfg.len(), 1);
        assert!(group_cfg.contains_key("queue1_group1"));

        let group_fvs = &group_cfg["queue1_group1"];
        assert_eq!(
            group_fvs
                .iter()
                .find(|(k, _)| k == "queue")
                .map(|(_, v)| v.as_str()),
            Some("1")
        );
        assert_eq!(
            group_fvs
                .iter()
                .find(|(k, _)| k == "cir")
                .map(|(_, v)| v.as_str()),
            Some("600")
        );
    }

    #[test]
    fn test_parse_copp_init_file_empty() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "{{}}").unwrap();
        file.flush().unwrap();

        let (trap_cfg, group_cfg) = parse_copp_init_file(file.path().to_str().unwrap()).unwrap();

        assert!(trap_cfg.is_empty());
        assert!(group_cfg.is_empty());
    }

    #[test]
    fn test_parse_copp_init_file_not_found() {
        let result = parse_copp_init_file("/nonexistent/copp_cfg.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_copp_init_file_invalid_json() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "{{invalid json").unwrap();
        file.flush().unwrap();

        let result = parse_copp_init_file(file.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_field_values_number() {
        let json: Value = serde_json::from_str(r#"{"queue": 1, "cir": 600}"#).unwrap();
        let fvs = parse_field_values(&json).unwrap();

        assert_eq!(fvs.len(), 2);
        assert_eq!(
            fvs.iter()
                .find(|(k, _)| k == "queue")
                .map(|(_, v)| v.as_str()),
            Some("1")
        );
        assert_eq!(
            fvs.iter()
                .find(|(k, _)| k == "cir")
                .map(|(_, v)| v.as_str()),
            Some("600")
        );
    }

    #[test]
    fn test_parse_field_values_bool() {
        let json: Value = serde_json::from_str(r#"{"enabled": true}"#).unwrap();
        let fvs = parse_field_values(&json).unwrap();

        assert_eq!(fvs.len(), 1);
        assert_eq!(
            fvs.iter()
                .find(|(k, _)| k == "enabled")
                .map(|(_, v)| v.as_str()),
            Some("true")
        );
    }
}
