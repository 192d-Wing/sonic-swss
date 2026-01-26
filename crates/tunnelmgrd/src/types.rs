//! Tunnel type definitions and constants

/// Tunnel type identifier for IP-in-IP tunnels
pub const TUNNEL_TYPE_IPINIP: &str = "IPINIP";

/// Tunnel interface name
pub const TUNNEL_INTERFACE: &str = "tun0";

/// Loopback interface used as tunnel source
pub const LOOPBACK_SRC: &str = "Loopback3";

/// Simple IP prefix representation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpPrefix {
    prefix: String,
}

impl IpPrefix {
    pub fn new(prefix: String) -> Self {
        Self { prefix }
    }

    pub fn is_v4(&self) -> bool {
        !self.prefix.contains(':')
    }

    pub fn is_v6(&self) -> bool {
        self.prefix.contains(':')
    }
}

impl std::fmt::Display for IpPrefix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.prefix)
    }
}

impl std::str::FromStr for IpPrefix {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Basic validation - just check if it has a slash
        if !s.contains('/') {
            return Err(format!("Invalid IP prefix: {}", s));
        }
        Ok(Self::new(s.to_string()))
    }
}

/// Tunnel information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelInfo {
    /// Tunnel type (e.g., "IPINIP")
    pub tunnel_type: String,
    /// Local endpoint IP (from CONFIG_DB dst_ip field)
    pub dst_ip: String,
    /// Remote endpoint IP (from PEER_SWITCH table)
    pub remote_ip: String,
    /// Optional source IP for P2P tunnels
    pub src_ip: Option<String>,
}

impl TunnelInfo {
    /// Create a new TunnelInfo with type and destination IP
    pub fn new(tunnel_type: String, dst_ip: String) -> Self {
        Self {
            tunnel_type,
            dst_ip,
            remote_ip: String::new(),
            src_ip: None,
        }
    }

    /// Set the remote IP (builder pattern)
    pub fn with_remote_ip(mut self, remote_ip: String) -> Self {
        self.remote_ip = remote_ip;
        self
    }

    /// Set the source IP (builder pattern)
    pub fn with_src_ip(mut self, src_ip: Option<String>) -> Self {
        self.src_ip = src_ip;
        self
    }

    /// Returns true if this is a P2P tunnel (has source IP)
    pub fn is_p2p(&self) -> bool {
        self.src_ip.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_info_builder() {
        let info = TunnelInfo::new("IPINIP".to_string(), "10.1.0.32".to_string())
            .with_remote_ip("10.1.0.33".to_string())
            .with_src_ip(Some("10.0.0.1".to_string()));

        assert_eq!(info.tunnel_type, "IPINIP");
        assert_eq!(info.dst_ip, "10.1.0.32");
        assert_eq!(info.remote_ip, "10.1.0.33");
        assert!(info.is_p2p());
    }

    #[test]
    fn test_tunnel_info_p2mp() {
        let info = TunnelInfo::new("IPINIP".to_string(), "10.1.0.32".to_string())
            .with_remote_ip("10.1.0.33".to_string());

        assert!(!info.is_p2p());
        assert_eq!(info.src_ip, None);
    }

    #[test]
    fn test_tunnel_type_constant() {
        assert_eq!(TUNNEL_TYPE_IPINIP, "IPINIP");
        assert_eq!(TUNNEL_INTERFACE, "tun0");
        assert_eq!(LOOPBACK_SRC, "Loopback3");
    }
}
