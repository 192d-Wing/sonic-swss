//! VXLAN tunnel types.

use std::collections::HashMap;
use std::net::IpAddr;

pub type RawSaiObjectId = u64;
pub type Vni = u32;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VxlanTunnelKey {
    pub src_ip: IpAddr,
    pub dst_ip: IpAddr,
}

impl VxlanTunnelKey {
    pub fn new(src_ip: IpAddr, dst_ip: IpAddr) -> Self {
        Self { src_ip, dst_ip }
    }
}

#[derive(Debug, Clone)]
pub struct VxlanTunnelConfig {
    pub src_ip: IpAddr,
    pub dst_ip: IpAddr,
    pub tunnel_name: String,
}

#[derive(Debug, Clone)]
pub struct VxlanTunnelEntry {
    pub key: VxlanTunnelKey,
    pub config: VxlanTunnelConfig,
    pub tunnel_oid: RawSaiObjectId,
    pub encap_mapper_oid: RawSaiObjectId,
    pub decap_mapper_oid: RawSaiObjectId,
}

impl VxlanTunnelEntry {
    pub fn new(config: VxlanTunnelConfig) -> Self {
        let key = VxlanTunnelKey::new(config.src_ip, config.dst_ip);
        Self {
            key,
            config,
            tunnel_oid: 0,
            encap_mapper_oid: 0,
            decap_mapper_oid: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VxlanVrfMapKey {
    pub vni: Vni,
    pub vrf_name: String,
}

impl VxlanVrfMapKey {
    pub fn new(vni: Vni, vrf_name: String) -> Self {
        Self { vni, vrf_name }
    }
}

#[derive(Debug, Clone)]
pub struct VxlanVrfMapEntry {
    pub key: VxlanVrfMapKey,
    pub vrf_oid: RawSaiObjectId,
}

impl VxlanVrfMapEntry {
    pub fn new(key: VxlanVrfMapKey) -> Self {
        Self {
            key,
            vrf_oid: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VxlanVlanMapKey {
    pub vni: Vni,
    pub vlan_id: u16,
}

impl VxlanVlanMapKey {
    pub fn new(vni: Vni, vlan_id: u16) -> Self {
        Self { vni, vlan_id }
    }
}

#[derive(Debug, Clone)]
pub struct VxlanVlanMapEntry {
    pub key: VxlanVlanMapKey,
    pub vlan_oid: RawSaiObjectId,
    pub bridge_port_oid: RawSaiObjectId,
}

impl VxlanVlanMapEntry {
    pub fn new(key: VxlanVlanMapKey) -> Self {
        Self {
            key,
            vlan_oid: 0,
            bridge_port_oid: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VxlanEncapType {
    L2,
    L3,
}

#[derive(Debug, Clone, Default)]
pub struct VxlanStats {
    pub tunnels_created: u64,
    pub vrf_maps_created: u64,
    pub vlan_maps_created: u64,
}
