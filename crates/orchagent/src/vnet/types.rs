//! VNET (Virtual Network) types.

use std::collections::HashMap;
use std::net::IpAddr;

pub type RawSaiObjectId = u64;
pub type Vni = u32;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VnetKey {
    pub vnet_name: String,
}

impl VnetKey {
    pub fn new(vnet_name: String) -> Self {
        Self { vnet_name }
    }
}

#[derive(Debug, Clone)]
pub struct VnetConfig {
    pub vnet_name: String,
    pub vni: Option<Vni>,
    pub vxlan_tunnel: Option<String>,
    pub scope: Option<String>,
    pub advertise_prefix: bool,
}

#[derive(Debug, Clone)]
pub struct VnetEntry {
    pub key: VnetKey,
    pub config: VnetConfig,
    pub vrf_oid: RawSaiObjectId,
    pub vnet_oid: RawSaiObjectId,
}

impl VnetEntry {
    pub fn new(config: VnetConfig) -> Self {
        let key = VnetKey::new(config.vnet_name.clone());
        Self {
            key,
            config,
            vrf_oid: 0,
            vnet_oid: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VnetRouteKey {
    pub vnet_name: String,
    pub prefix: String,
}

impl VnetRouteKey {
    pub fn new(vnet_name: String, prefix: String) -> Self {
        Self { vnet_name, prefix }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VnetRouteType {
    Direct,
    Tunnel,
    Vnet,
}

#[derive(Debug, Clone)]
pub struct VnetRouteConfig {
    pub route_type: VnetRouteType,
    pub endpoint: Option<IpAddr>,
    pub endpoint_monitor: Option<IpAddr>,
    pub mac_address: Option<String>,
    pub vni: Option<Vni>,
    pub peer_list: Vec<IpAddr>,
}

#[derive(Debug, Clone)]
pub struct VnetRouteEntry {
    pub key: VnetRouteKey,
    pub config: VnetRouteConfig,
    pub route_oid: RawSaiObjectId,
    pub nh_oid: RawSaiObjectId,
}

impl VnetRouteEntry {
    pub fn new(key: VnetRouteKey, config: VnetRouteConfig) -> Self {
        Self {
            key,
            config,
            route_oid: 0,
            nh_oid: 0,
        }
    }

    pub fn is_tunnel_route(&self) -> bool {
        self.config.route_type == VnetRouteType::Tunnel
    }

    pub fn is_vnet_route(&self) -> bool {
        self.config.route_type == VnetRouteType::Vnet
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VnetBridgePortKey {
    pub vnet_name: String,
    pub bridge_name: String,
}

impl VnetBridgePortKey {
    pub fn new(vnet_name: String, bridge_name: String) -> Self {
        Self { vnet_name, bridge_name }
    }
}

#[derive(Debug, Clone)]
pub struct VnetBridgePortEntry {
    pub key: VnetBridgePortKey,
    pub bridge_port_oid: RawSaiObjectId,
}

impl VnetBridgePortEntry {
    pub fn new(key: VnetBridgePortKey) -> Self {
        Self {
            key,
            bridge_port_oid: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VnetStats {
    pub vnets_created: u64,
    pub routes_created: u64,
    pub bridge_ports_created: u64,
}
