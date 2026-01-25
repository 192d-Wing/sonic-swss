//! Fine-Grained Next Hop Group types.

use std::collections::{HashMap, HashSet};

pub type RawSaiObjectId = u64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FgNhgPrefix {
    pub ip_prefix: String,
}

impl FgNhgPrefix {
    pub fn new(ip_prefix: String) -> Self {
        Self { ip_prefix }
    }
}

#[derive(Debug, Clone)]
pub struct FgNhgEntry {
    pub prefix: FgNhgPrefix,
    pub next_hops: Vec<FgNextHop>,
    pub nhg_oid: RawSaiObjectId,
    pub bucket_size: u32,
}

impl FgNhgEntry {
    pub fn new(prefix: FgNhgPrefix, bucket_size: u32) -> Self {
        Self {
            prefix,
            next_hops: Vec::new(),
            nhg_oid: 0,
            bucket_size,
        }
    }

    pub fn add_next_hop(&mut self, nh: FgNextHop) {
        self.next_hops.push(nh);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FgNextHop {
    pub ip: String,
    pub interface: String,
    pub weight: u32,
}

impl FgNextHop {
    pub fn new(ip: String, interface: String, weight: u32) -> Self {
        Self {
            ip,
            interface,
            weight,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FgNhgMemberConfig {
    pub next_hop: FgNextHop,
    pub link_selection_map: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct FgNhgMemberEntry {
    pub config: FgNhgMemberConfig,
    pub member_oid: RawSaiObjectId,
}

impl FgNhgMemberEntry {
    pub fn new(config: FgNhgMemberConfig) -> Self {
        Self {
            config,
            member_oid: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BankSelectionMode {
    Static,
    Dynamic,
}

#[derive(Debug, Clone)]
pub struct FgNhgBankConfig {
    pub bank_id: u32,
    pub selection_mode: BankSelectionMode,
    pub members: HashSet<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FgNhgStats {
    pub nhgs_created: u64,
    pub members_added: u64,
    pub rebalances: u64,
}
