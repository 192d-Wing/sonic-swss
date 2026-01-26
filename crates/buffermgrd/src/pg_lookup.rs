//! PG profile lookup file parser

use std::fs::File;
use std::io::{BufRead, BufReader};

use sonic_cfgmgr_common::CfgMgrResult;
use tracing::info;

use crate::types::{PgProfile, PgProfileLookup};

/// Parse PG profile lookup file
///
/// File format:
/// ```text
/// # speed cable size    xon   xoff threshold xon_offset
/// 40000   5m    34816   18432 16384 1        2496
/// 100000  300m  184320  18432 165888 1
/// ```
///
/// Lines starting with '#' are comments.
/// Empty lines are ignored.
/// xon_offset is optional (defaults to empty string).
pub fn parse_pg_lookup_file(path: &str) -> CfgMgrResult<PgProfileLookup> {
    let file = File::open(path).map_err(|e| {
        sonic_cfgmgr_common::CfgMgrError::internal(format!(
            "Failed to open PG lookup file {}: {}",
            path, e
        ))
    })?;

    let reader = BufReader::new(file);
    let mut lookup = PgProfileLookup::new();

    for line in reader.lines() {
        let line = line.map_err(|e| {
            sonic_cfgmgr_common::CfgMgrError::internal(format!("Failed to read line: {}", e))
        })?;

        // Skip empty lines and comments
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some((speed, cable, profile)) = PgProfile::from_line(trimmed) {
            lookup
                .entry(speed.clone())
                .or_default()
                .insert(cable.clone(), profile.clone());

            info!(
                "PG profile for speed {} and cable {}: size={}, xon={}, xoff={}, th={}, xon_offset={}",
                speed, cable, profile.size, profile.xon, profile.xoff,
                profile.threshold, profile.xon_offset
            );
        }
    }

    Ok(lookup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_pg_lookup_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "# Comment line").unwrap();
        writeln!(file, "40000 5m 34816 18432 16384 1 2496").unwrap();
        writeln!(file, "100000 300m 184320 18432 165888 1").unwrap();
        writeln!(file, "").unwrap(); // Empty line
        file.flush().unwrap();

        let lookup = parse_pg_lookup_file(file.path().to_str().unwrap()).unwrap();

        assert!(lookup.contains_key("40000"));
        assert!(lookup["40000"].contains_key("5m"));
        assert_eq!(lookup["40000"]["5m"].size, "34816");
        assert_eq!(lookup["40000"]["5m"].xon_offset, "2496");

        assert!(lookup.contains_key("100000"));
        assert_eq!(lookup["100000"]["300m"].xon_offset, "");
    }

    #[test]
    fn test_parse_pg_lookup_file_comments_only() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "# Comment 1").unwrap();
        writeln!(file, "# Comment 2").unwrap();
        file.flush().unwrap();

        let lookup = parse_pg_lookup_file(file.path().to_str().unwrap()).unwrap();
        assert!(lookup.is_empty());
    }

    #[test]
    fn test_parse_pg_lookup_file_invalid_line() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "40000 5m 34816 18432 16384 1 2496").unwrap();
        writeln!(file, "invalid line").unwrap(); // Invalid - will be skipped
        writeln!(file, "100000 300m 184320 18432 165888 1").unwrap();
        file.flush().unwrap();

        let lookup = parse_pg_lookup_file(file.path().to_str().unwrap()).unwrap();

        // Should have 2 valid entries despite invalid line
        assert_eq!(lookup.len(), 2);
        assert!(lookup.contains_key("40000"));
        assert!(lookup.contains_key("100000"));
    }

    #[test]
    fn test_parse_pg_lookup_file_not_found() {
        let result = parse_pg_lookup_file("/nonexistent/file.txt");
        assert!(result.is_err());
    }
}
