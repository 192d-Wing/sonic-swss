//! Buffer manager type definitions

use std::collections::HashMap;

/// PG profile buffer parameters
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgProfile {
    pub size: String,
    pub xon: String,
    pub xon_offset: String,
    pub xoff: String,
    pub threshold: String,
}

impl PgProfile {
    /// Parse PG profile from lookup file line
    ///
    /// Format: speed cable size xon xoff threshold [xon_offset]
    /// Example: "40000 5m 34816 18432 16384 1 2496"
    pub fn from_line(line: &str) -> Option<(String, String, Self)> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 6 {
            return None;
        }

        let speed = parts[0].to_string();
        let cable = parts[1].to_string();
        let profile = Self {
            size: parts[2].to_string(),
            xon: parts[3].to_string(),
            xoff: parts[4].to_string(),
            threshold: parts[5].to_string(),
            xon_offset: parts.get(6).unwrap_or(&"").to_string(),
        };

        Some((speed, cable, profile))
    }
}

/// Nested lookup: [speed][cable] -> PgProfile
pub type PgProfileLookup = HashMap<String, HashMap<String, PgProfile>>;

/// Port cable length mapping
pub type PortCableLength = HashMap<String, String>;

/// Port speed mapping
pub type PortSpeed = HashMap<String, String>;

/// Port PFC status mapping (PFC enable string like "3,4")
pub type PortPfcStatus = HashMap<String, String>;

/// Port admin status mapping ("up" or "down")
pub type PortAdminStatus = HashMap<String, String>;

/// Platform type for platform-specific behavior
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Platform {
    Mellanox,
    Barefoot,
    Other(String),
}

impl Platform {
    /// Detect platform from ASIC_VENDOR environment variable
    pub fn from_env() -> Self {
        match std::env::var("ASIC_VENDOR") {
            Ok(val) if val == "mellanox" => Platform::Mellanox,
            Ok(val) if val == "barefoot" => Platform::Barefoot,
            Ok(val) => Platform::Other(val),
            Err(_) => Platform::Other("unknown".to_string()),
        }
    }

    /// Check if platform is Mellanox
    pub fn is_mellanox(&self) -> bool {
        matches!(self, Platform::Mellanox)
    }

    /// Check if platform is Barefoot
    pub fn is_barefoot(&self) -> bool {
        matches!(self, Platform::Barefoot)
    }

    /// Check if platform is Mellanox or Barefoot (for special handling)
    pub fn is_mellanox_or_barefoot(&self) -> bool {
        self.is_mellanox() || self.is_barefoot()
    }
}

/// Buffer pool name constant
pub const INGRESS_LOSSLESS_PG_POOL_NAME: &str = "ingress_lossless_pool";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pg_profile_from_line() {
        let line = "40000 5m 34816 18432 16384 1 2496";
        let (speed, cable, profile) = PgProfile::from_line(line).unwrap();

        assert_eq!(speed, "40000");
        assert_eq!(cable, "5m");
        assert_eq!(profile.size, "34816");
        assert_eq!(profile.xon, "18432");
        assert_eq!(profile.xoff, "16384");
        assert_eq!(profile.threshold, "1");
        assert_eq!(profile.xon_offset, "2496");
    }

    #[test]
    fn test_pg_profile_from_line_no_offset() {
        let line = "100000 300m 184320 18432 165888 1";
        let (_, _, profile) = PgProfile::from_line(line).unwrap();

        assert_eq!(profile.xon_offset, "");
    }

    #[test]
    fn test_pg_profile_from_line_invalid() {
        let line = "40000 5m";
        assert!(PgProfile::from_line(line).is_none());
    }

    #[test]
    fn test_platform_from_env() {
        std::env::set_var("ASIC_VENDOR", "mellanox");
        let platform = Platform::from_env();
        assert!(platform.is_mellanox());
        assert!(platform.is_mellanox_or_barefoot());

        std::env::set_var("ASIC_VENDOR", "barefoot");
        let platform = Platform::from_env();
        assert!(platform.is_barefoot());
        assert!(platform.is_mellanox_or_barefoot());

        std::env::set_var("ASIC_VENDOR", "broadcom");
        let platform = Platform::from_env();
        assert!(!platform.is_mellanox());
        assert!(!platform.is_barefoot());
        assert!(!platform.is_mellanox_or_barefoot());
    }

    #[test]
    fn test_platform_unknown() {
        std::env::remove_var("ASIC_VENDOR");
        let platform = Platform::from_env();
        assert_eq!(platform, Platform::Other("unknown".to_string()));
    }
}
