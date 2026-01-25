//! Netlink socket handling for neighbor events
//!
//! # NIST 800-53 Rev 5 Control Mappings
//! - SC-7: Boundary Protection - Kernel interface for network state
//! - SI-4: System Monitoring - Monitor neighbor table changes
//! - AU-12: Audit Record Generation - Log all neighbor events

#[cfg(target_os = "linux")]
mod linux {
    use crate::error::{NeighsyncError, Result};
    use crate::types::{
        MacAddress, NeighborEntry, NeighborFlags, NeighborMessageType, NeighborState,
    };
    use netlink_packet_core::{NetlinkMessage, NetlinkPayload};
    use netlink_packet_route::RouteNetlinkMessage;
    use netlink_packet_route::neighbour::{NeighbourAddress, NeighbourAttribute, NeighbourMessage};
    use netlink_sys::{Socket, SocketAddr, protocols::NETLINK_ROUTE};
    use std::collections::HashMap;
    use std::net::IpAddr;
    use std::os::fd::AsRawFd;
    use tracing::{debug, instrument, trace, warn};

    /// Netlink group for neighbor notifications (RTNLGRP_NEIGH = 3)
    const RTNLGRP_NEIGH: u32 = 3;

    /// Interface name cache
    ///
    /// # NIST Controls
    /// - CM-8: System Component Inventory - Track interface names
    #[derive(Debug, Default)]
    pub struct InterfaceCache {
        cache: HashMap<u32, String>,
    }

    impl InterfaceCache {
        /// Look up interface name by index
        pub fn get(&self, ifindex: u32) -> Option<&str> {
            self.cache.get(&ifindex).map(|s| s.as_str())
        }

        /// Add interface to cache
        pub fn insert(&mut self, ifindex: u32, name: String) {
            self.cache.insert(ifindex, name);
        }

        /// Resolve interface name, querying system if not cached
        ///
        /// # NIST Controls
        /// - CM-8: System Component Inventory - Interface resolution
        pub fn resolve(&mut self, ifindex: u32) -> Result<&str> {
            if !self.cache.contains_key(&ifindex) {
                // Use nix to get interface name
                match nix::net::if_::if_indextoname(ifindex) {
                    Ok(name) => {
                        let name_str = name.to_string_lossy().into_owned();
                        self.cache.insert(ifindex, name_str);
                    }
                    Err(_) => {
                        return Err(NeighsyncError::InterfaceNotFound(ifindex));
                    }
                }
            }
            Ok(self.cache.get(&ifindex).unwrap())
        }
    }

    /// Netlink socket for receiving neighbor events
    ///
    /// # NIST Controls
    /// - SC-7: Boundary Protection - Kernel netlink interface
    /// - SI-4: System Monitoring - Event-driven monitoring
    pub struct NetlinkSocket {
        socket: Socket,
        buffer: Vec<u8>,
        interface_cache: InterfaceCache,
    }

    impl NetlinkSocket {
        /// Create and bind a new netlink socket for neighbor events
        ///
        /// # NIST Controls
        /// - AC-3: Access Enforcement - Kernel socket requires CAP_NET_ADMIN
        #[instrument]
        pub fn new() -> Result<Self> {
            let mut socket = Socket::new(NETLINK_ROUTE)
                .map_err(|e| NeighsyncError::Netlink(format!("Failed to create socket: {}", e)))?;

            // Subscribe to neighbor events
            let groups = 1 << (RTNLGRP_NEIGH - 1);
            let addr = SocketAddr::new(0, groups);
            socket
                .bind(&addr)
                .map_err(|e| NeighsyncError::Netlink(format!("Failed to bind socket: {}", e)))?;

            debug!("Netlink socket bound to RTNLGRP_NEIGH");

            Ok(Self {
                socket,
                buffer: vec![0u8; 65536],
                interface_cache: InterfaceCache::default(),
            })
        }

        /// Get the raw file descriptor for async polling
        pub fn as_raw_fd(&self) -> i32 {
            self.socket.as_raw_fd()
        }

        /// Request a dump of the current neighbor table
        ///
        /// # NIST Controls
        /// - CP-10: System Recovery - Initial state dump for warm restart
        #[instrument(skip(self))]
        pub fn request_dump(&mut self) -> Result<()> {
            use netlink_packet_core::{NetlinkFlags, NetlinkHeader};
            use netlink_packet_route::RouteNetlinkMessage;

            let mut header = NetlinkHeader::default();
            header.flags = NetlinkFlags::REQUEST | NetlinkFlags::DUMP;

            // Create RTM_GETNEIGH message
            let msg = NeighbourMessage::default();
            let payload = RouteNetlinkMessage::GetNeighbour(msg);
            let mut packet = NetlinkMessage::new(header, NetlinkPayload::InnerMessage(payload));
            packet.finalize();

            let bytes = packet.buffer_len();
            let mut buf = vec![0u8; bytes];
            packet.serialize(&mut buf);

            self.socket.send(&buf, 0).map_err(|e| {
                NeighsyncError::Netlink(format!("Failed to send dump request: {}", e))
            })?;

            debug!("Requested neighbor table dump");
            Ok(())
        }

        /// Receive and parse neighbor events
        ///
        /// # NIST Controls
        /// - SI-4: System Monitoring - Process kernel events
        /// - AU-12: Audit Record Generation - Events for audit
        #[instrument(skip(self))]
        pub fn receive_events(&mut self) -> Result<Vec<(NeighborMessageType, NeighborEntry)>> {
            let len = self
                .socket
                .recv(&mut self.buffer, 0)
                .map_err(|e| NeighsyncError::Netlink(format!("Failed to receive: {}", e)))?;

            let mut events = Vec::new();
            let mut offset = 0;

            while offset < len {
                let msg =
                    NetlinkMessage::<RouteNetlinkMessage>::deserialize(&self.buffer[offset..])
                        .map_err(|e| {
                            NeighsyncError::Netlink(format!("Failed to parse message: {}", e))
                        })?;

                offset += msg.header.length as usize;
                // Align to 4 bytes
                offset = (offset + 3) & !3;

                if let Some((msg_type, entry)) = self.parse_neighbor_message(&msg)? {
                    events.push((msg_type, entry));
                }
            }

            trace!(count = events.len(), "Received neighbor events");
            Ok(events)
        }

        /// Parse a netlink message into a neighbor entry
        fn parse_neighbor_message(
            &mut self,
            msg: &NetlinkMessage<RouteNetlinkMessage>,
        ) -> Result<Option<(NeighborMessageType, NeighborEntry)>> {
            let (msg_type, neigh_msg) = match &msg.payload {
                NetlinkPayload::InnerMessage(RouteNetlinkMessage::NewNeighbour(n)) => {
                    (NeighborMessageType::New, n)
                }
                NetlinkPayload::InnerMessage(RouteNetlinkMessage::DelNeighbour(n)) => {
                    (NeighborMessageType::Delete, n)
                }
                NetlinkPayload::InnerMessage(RouteNetlinkMessage::GetNeighbour(n)) => {
                    (NeighborMessageType::Get, n)
                }
                _ => return Ok(None),
            };

            // Extract fields from NeighbourMessage header
            let family = neigh_msg.header.family;
            let ifindex = neigh_msg.header.ifindex;
            let state = NeighborState::from_kernel(neigh_msg.header.state);
            let flags = NeighborFlags::from_kernel(neigh_msg.header.flags);

            // Filter by address family
            #[cfg(not(feature = "ipv4"))]
            if family as i32 != libc::AF_INET6 {
                trace!(family, "Ignoring non-IPv6 neighbor (IPv4 disabled)");
                return Ok(None);
            }

            #[cfg(feature = "ipv4")]
            if family as i32 != libc::AF_INET && family as i32 != libc::AF_INET6 {
                trace!(family, "Ignoring non-IP neighbor");
                return Ok(None);
            }

            // Extract IP and MAC from attributes
            let mut ip: Option<IpAddr> = None;
            let mut mac: Option<MacAddress> = None;

            for attr in &neigh_msg.attributes {
                match attr {
                    NeighbourAttribute::Destination(addr) => {
                        ip = Some(parse_neigh_address(addr));
                    }
                    NeighbourAttribute::LinkLocalAddress(bytes) => {
                        if bytes.len() == 6 {
                            let mut arr = [0u8; 6];
                            arr.copy_from_slice(bytes);
                            mac = Some(MacAddress::new(arr));
                        }
                    }
                    _ => {}
                }
            }

            let Some(ip) = ip else {
                trace!("Neighbor message missing IP address");
                return Ok(None);
            };

            // For delete messages, MAC may not be present
            let mac = mac.unwrap_or(MacAddress::ZERO);

            // Resolve interface name
            let interface = self.interface_cache.resolve(ifindex)?.to_string();

            let entry = NeighborEntry {
                ifindex,
                interface,
                ip,
                mac,
                state,
                externally_learned: flags.ext_learned,
            };

            debug!(
                msg_type = ?msg_type,
                interface = %entry.interface,
                ip = %entry.ip,
                mac = %entry.mac,
                state = ?entry.state,
                "Parsed neighbor event"
            );

            Ok(Some((msg_type, entry)))
        }
    }

    /// Parse NeighbourAddress to IpAddr
    fn parse_neigh_address(addr: &NeighbourAddress) -> IpAddr {
        match addr {
            NeighbourAddress::Inet(ipv4) => IpAddr::V4(*ipv4),
            NeighbourAddress::Inet6(ipv6) => IpAddr::V6(*ipv6),
            _ => panic!("Unexpected address type"),
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::*;

/// Mock implementation for non-Linux platforms (development only)
#[cfg(not(target_os = "linux"))]
mod mock {
    use crate::error::Result;
    use crate::types::{NeighborEntry, NeighborMessageType};

    #[derive(Debug, Default)]
    pub struct InterfaceCache;

    impl InterfaceCache {
        #[allow(unused_variables)]
        pub fn resolve(&mut self, ifindex: u32) -> Result<&str> {
            Ok("mock0")
        }
    }

    pub struct NetlinkSocket;

    impl NetlinkSocket {
        pub fn new() -> Result<Self> {
            Ok(Self)
        }

        pub fn as_raw_fd(&self) -> i32 {
            -1
        }

        pub fn request_dump(&mut self) -> Result<()> {
            Ok(())
        }

        pub fn receive_events(&mut self) -> Result<Vec<(NeighborMessageType, NeighborEntry)>> {
            Ok(Vec::new())
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub use mock::*;
