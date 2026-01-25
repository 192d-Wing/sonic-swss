//! Kernel netlink socket for real-time port state events
//!
//! This module provides real kernel netlink socket integration for receiving
//! RTM_NEWLINK and RTM_DELLINK messages from the kernel on Linux.
//! On non-Linux platforms (macOS, etc.), uses a mock implementation for development.
//!
//! Supports EOIU (End of Init sequence User indication) detection for warm restart
//! coordination.
//!
//! NIST 800-53 Rev5 [SI-4]: System Monitoring - Real-time state monitoring

use crate::eoiu_detector::EoiuDetector;
use crate::error::{PortsyncError, Result};
use crate::port_sync::NetlinkEvent;

#[cfg(target_os = "linux")]
use nix::sys::socket::{AddressFamily, SockFlag, SockProtocol, SockType, socket};
#[cfg(target_os = "linux")]
use std::os::unix::io::RawFd;

/// Netlink socket for kernel RTM_LINK events
///
/// Linux: Receives RTM_NEWLINK and RTM_DELLINK messages from kernel via netlink socket.
/// Other platforms: Mock implementation for development/testing.
#[derive(Debug)]
pub struct NetlinkSocket {
    /// Is the socket connected?
    connected: bool,

    /// Linux: Raw socket file descriptor
    #[cfg(target_os = "linux")]
    fd: Option<std::os::unix::io::RawFd>,
    #[cfg(target_os = "linux")]
    buffer: Vec<u8>,

    /// Non-Linux: Mock event queue for testing
    #[cfg(not(target_os = "linux"))]
    mock_events: Vec<NetlinkEvent>,

    /// EOIU detector for warm restart coordination
    eoiu_detector: EoiuDetector,
}

impl NetlinkSocket {
    /// Create new netlink socket
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            Ok(Self {
                connected: false,
                fd: None,
                buffer: vec![0u8; 8192],
                eoiu_detector: EoiuDetector::new(),
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok(Self {
                connected: false,
                mock_events: Vec::new(),
                eoiu_detector: EoiuDetector::new(),
            })
        }
    }

    /// Connect to kernel netlink subsystem
    #[cfg(target_os = "linux")]
    pub fn connect(&mut self) -> Result<()> {
        // Create netlink socket for NETLINK_ROUTE protocol
        let fd = socket(
            AddressFamily::Netlink,
            SockType::Raw,
            SockFlag::empty(),
            Some(SockProtocol::NetlinkRoute),
        )
        .map_err(|e| PortsyncError::Netlink(format!("Failed to create netlink socket: {}", e)))?;

        // Set non-blocking mode for event-driven processing
        nix::fcntl::fcntl(
            fd,
            nix::fcntl::FcntlArg::SetFlags(nix::fcntl::OFlag::O_NONBLOCK),
        )
        .map_err(|e| PortsyncError::Netlink(format!("Failed to set non-blocking: {}", e)))?;

        eprintln!("portsyncd: Connected to netlink socket");
        self.fd = Some(fd);
        self.connected = true;
        Ok(())
    }

    /// Connect to kernel netlink subsystem (mock for non-Linux)
    #[cfg(not(target_os = "linux"))]
    pub fn connect(&mut self) -> Result<()> {
        eprintln!("portsyncd: Connected to netlink socket (mock)");
        self.connected = true;
        Ok(())
    }

    /// Check if connected to netlink
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Check if EOIU signal has been detected
    pub fn is_eoiu_detected(&self) -> bool {
        self.eoiu_detector.is_detected()
    }

    /// Get EOIU detector
    pub fn eoiu_detector(&self) -> &EoiuDetector {
        &self.eoiu_detector
    }

    /// Get mutable EOIU detector
    pub fn eoiu_detector_mut(&mut self) -> &mut EoiuDetector {
        &mut self.eoiu_detector
    }

    /// Receive next netlink event from kernel
    #[cfg(target_os = "linux")]
    pub fn receive_event(&mut self) -> Result<Option<NetlinkEvent>> {
        if !self.connected || self.fd.is_none() {
            return Err(PortsyncError::Netlink(
                "Not connected to netlink socket".to_string(),
            ));
        }

        let fd = self.fd.ok_or_else(|| {
            PortsyncError::Netlink("Socket file descriptor not available".to_string())
        })?;

        // Try to read netlink message from socket
        match nix::sys::socket::recv(fd, &mut self.buffer, nix::sys::socket::MsgFlags::empty()) {
            Ok(n) if n > 0 => {
                // Parse the received netlink message
                match parse_netlink_message(&self.buffer[..n]) {
                    Ok((event, ifi_change)) => {
                        // Check for EOIU signal during warm restart
                        let _ = self.eoiu_detector.check_eoiu(
                            &event.port_name,
                            ifi_change,
                            event.flags.unwrap_or(0),
                        );
                        Ok(Some(event))
                    }
                    Err(_) => Ok(None), // Ignore parsing errors, try next message
                }
            }
            Ok(_) => Ok(None), // No data received
            Err(nix::Error::EAGAIN) | Err(nix::Error::EWOULDBLOCK) => {
                Ok(None) // No data available in non-blocking mode
            }
            Err(e) => Err(PortsyncError::Netlink(format!(
                "Failed to receive from netlink: {}",
                e
            ))),
        }
    }

    /// Receive next netlink event (mock for non-Linux)
    #[cfg(not(target_os = "linux"))]
    pub fn receive_event(&mut self) -> Result<Option<NetlinkEvent>> {
        if !self.connected {
            return Err(PortsyncError::Netlink(
                "Not connected to netlink socket".to_string(),
            ));
        }

        // Return mock events in order, then None
        Ok(self.mock_events.pop())
    }

    /// Close netlink socket
    pub fn close(&mut self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            if let Some(fd) = self.fd {
                let _ = nix::unistd::close(fd);
                self.fd = None;
            }
        }

        self.connected = false;
        Ok(())
    }

    /// Get raw socket file descriptor
    pub fn fd(&self) -> i32 {
        #[cfg(target_os = "linux")]
        {
            self.fd.map(|fd| fd as i32).unwrap_or(-1)
        }

        #[cfg(not(target_os = "linux"))]
        {
            -1
        }
    }
}

/// Parse netlink message buffer into NetlinkEvent with ifi_change for EOIU detection (Linux only)
#[cfg(target_os = "linux")]
fn parse_netlink_message(buffer: &[u8]) -> Result<(NetlinkEvent, u32)> {
    use netlink_packet_core::{NetlinkMessage, NetlinkPayload};
    use netlink_packet_route::RouteNetlinkMessage;

    // Parse the buffer as netlink message
    let msg: NetlinkMessage<RouteNetlinkMessage> =
        netlink_packet_core::NetlinkMessage::deserialize(buffer).map_err(|e| {
            PortsyncError::Netlink(format!("Failed to parse netlink message: {}", e))
        })?;

    match msg.payload {
        NetlinkPayload::InnerMessage(RouteNetlinkMessage::NewLink(link_msg)) => {
            extract_netlink_event(link_msg, crate::port_sync::NetlinkEventType::NewLink)
        }
        NetlinkPayload::InnerMessage(RouteNetlinkMessage::DelLink(link_msg)) => {
            extract_netlink_event(link_msg, crate::port_sync::NetlinkEventType::DelLink)
        }
        _ => Err(PortsyncError::Netlink(
            "Unexpected netlink message type".to_string(),
        )),
    }
}

/// Extract port information from netlink link message (Linux only)
#[cfg(target_os = "linux")]
fn extract_netlink_event(
    link: netlink_packet_route::link::LinkMessage,
    event_type: crate::port_sync::NetlinkEventType,
) -> Result<(NetlinkEvent, u32)> {
    use netlink_packet_route::link::LinkAttribute;

    let mut port_name = String::new();
    let mut flags = None;
    let mut mtu = None;

    // Parse link attributes
    for attr in link.attributes {
        match attr {
            LinkAttribute::IfName(name) => port_name = name,
            LinkAttribute::Mtu(m) => mtu = Some(m),
            _ => {}
        }
    }

    // Extract IFF_UP flag from link header
    let link_flags = link.header.flags;
    flags = Some(link_flags as u32);

    // Extract ifi_change field for EOIU detection
    let ifi_change = link.header.change;

    let event = NetlinkEvent {
        event_type,
        port_name,
        flags,
        mtu,
    };

    Ok((event, ifi_change))
}

impl Default for NetlinkSocket {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            #[cfg(target_os = "linux")]
            {
                Self {
                    connected: false,
                    fd: None,
                    buffer: vec![0u8; 8192],
                    eoiu_detector: EoiuDetector::new(),
                }
            }

            #[cfg(not(target_os = "linux"))]
            {
                Self {
                    connected: false,
                    mock_events: Vec::new(),
                    eoiu_detector: EoiuDetector::new(),
                }
            }
        })
    }
}

impl Drop for NetlinkSocket {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Parse netlink RTM_NEWLINK message (public interface for testing)
/// Returns (NetlinkEvent, ifi_change field for EOIU detection)
#[cfg(target_os = "linux")]
pub fn parse_newlink_message(buffer: &[u8]) -> Result<(NetlinkEvent, u32)> {
    parse_netlink_message(buffer)
}

/// Parse netlink RTM_NEWLINK message (mock for non-Linux)
#[cfg(not(target_os = "linux"))]
pub fn parse_newlink_message(_buffer: &[u8]) -> Result<NetlinkEvent> {
    Err(PortsyncError::Netlink(
        "Netlink parsing not available on non-Linux platforms".to_string(),
    ))
}

/// Parse netlink RTM_DELLINK message (public interface for testing)
#[cfg(target_os = "linux")]
pub fn parse_dellink_message(buffer: &[u8]) -> Result<String> {
    let (event, _ifi_change) = parse_netlink_message(buffer)?;
    Ok(event.port_name)
}

/// Parse netlink RTM_DELLINK message (mock for non-Linux)
#[cfg(not(target_os = "linux"))]
pub fn parse_dellink_message(_buffer: &[u8]) -> Result<String> {
    Err(PortsyncError::Netlink(
        "Netlink parsing not available on non-Linux platforms".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_netlink_socket_creation() {
        let socket = NetlinkSocket::new().unwrap();
        assert_eq!(socket.fd(), -1);
        assert!(!socket.is_connected());
    }

    #[test]
    fn test_netlink_socket_connect() {
        let mut socket = NetlinkSocket::new().unwrap();
        assert!(socket.connect().is_ok());
        assert!(socket.is_connected());
    }

    #[test]
    fn test_netlink_socket_receive_not_connected() {
        let mut socket = NetlinkSocket::new().unwrap();
        assert!(socket.receive_event().is_err());
    }

    #[test]
    fn test_netlink_socket_default() {
        let socket = NetlinkSocket::default();
        assert!(!socket.is_connected());
    }

    #[test]
    fn test_netlink_socket_close() {
        let mut socket = NetlinkSocket::new().unwrap();
        socket.connect().unwrap();
        assert!(socket.is_connected());
        socket.close().unwrap();
        assert!(!socket.is_connected());
    }

    #[test]
    fn test_netlink_receive_after_connect() {
        let mut socket = NetlinkSocket::new().unwrap();
        socket.connect().unwrap();
        let result = socket.receive_event();
        assert!(result.is_ok());
        // For now, returns None (no events in mock mode)
    }

    #[test]
    fn test_parse_newlink_not_implemented() {
        let buffer = vec![0u8; 64];
        let result = parse_newlink_message(&buffer);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_dellink_not_implemented() {
        let buffer = vec![0u8; 64];
        let result = parse_dellink_message(&buffer);
        assert!(result.is_err());
    }

    #[test]
    fn test_netlink_socket_eoiu_detector_creation() {
        let socket = NetlinkSocket::new().unwrap();
        assert!(!socket.is_eoiu_detected());
    }

    #[test]
    fn test_netlink_socket_eoiu_detector_access() {
        let mut socket = NetlinkSocket::new().unwrap();
        let detector = socket.eoiu_detector_mut();
        assert!(!detector.is_detected());
    }

    #[test]
    fn test_netlink_socket_eoiu_detector_immutable_access() {
        let socket = NetlinkSocket::new().unwrap();
        let detector = socket.eoiu_detector();
        assert!(!detector.is_detected());
    }

    #[test]
    fn test_netlink_socket_default_has_eoiu_detector() {
        let socket = NetlinkSocket::default();
        assert!(!socket.is_eoiu_detected());
    }
}
