# Week 6: vlanmgrd Planning Document

**Date**: 2026-01-25
**Phase**: Week 6 (First Medium-Complexity Manager)
**Status**: PLANNING
**C++ Reference**: [vlanmgr.cpp](../../../cfgmgr/vlanmgr.cpp) (1008 lines)

---

## Executive Summary

vlanmgrd is the first medium-complexity manager with:
- **Shell Commands**: Complex bash command chains for bridge/VLAN operations
- **State Management**: VLAN and VLAN member tracking
- **Warm Restart**: Replay lists for VLANs and members
- **Error Handling**: Port not ready, LAG race conditions
- **Multi-table Operations**: 4 CONFIG tables, 4 STATE tables, 4 APPL tables

**Complexity Assessment**: MEDIUM-HIGH
- C++ LOC: 1008
- Target Rust LOC: ~450-500 (55% reduction)
- Estimated Tests: 15-18
- Estimated Time: 3-4 days

---

## C++ Architecture Analysis

### Key Components

#### 1. Data Structures

```cpp
class VlanMgr : public Orch {
private:
    // Producers (write to APPL_DB)
    ProducerStateTable m_appVlanTableProducer;
    ProducerStateTable m_appVlanMemberTableProducer;
    ProducerStateTable m_appFdbTableProducer;
    ProducerStateTable m_appPortTableProducer;

    // Consumers (read from CONFIG_DB)
    Table m_cfgVlanTable;
    Table m_cfgVlanMemberTable;

    // State readers
    Table m_statePortTable;
    Table m_stateLagTable;
    Table m_stateVlanTable;
    Table m_stateVlanMemberTable;

    // Runtime state
    std::set<std::string> m_vlans;                    // Active VLANs
    std::set<std::string> m_vlanReplay;               // Warm restart VLANs
    std::set<std::string> m_vlanMemberReplay;         // Warm restart members
    bool replayDone;                                  // Warm restart complete

    // VLAN member tracking: port -> vlan -> tagging_mode
    std::unordered_map<std::string, std::unordered_map<std::string, std::string>> m_PortVlanMember;
};
```

#### 2. Shell Command Patterns

**Bridge Initialization** (constructor):
```bash
/bin/bash -c "/sbin/ip link del Bridge 2>/dev/null ;
               /sbin/ip link add Bridge up type bridge &&
               /sbin/ip link set Bridge mtu 9100 &&
               /sbin/ip link set Bridge address {{gMacAddress}} &&
               /sbin/bridge vlan del vid 1 dev Bridge self;
               /sbin/ip link del dummy 2>/dev/null;
               /sbin/ip link add dummy type dummy &&
               /sbin/ip link set dummy master Bridge &&
               /sbin/ip link set dummy up;
               /sbin/ip link set Bridge down &&
               /sbin/ip link set Bridge up"

/sbin/ip link set Bridge type bridge vlan_filtering 1
/sbin/ip link set Bridge type bridge no_linklocal_learn 1
```

**Add VLAN** (addHostVlan):
```bash
/bin/bash -c "/sbin/bridge vlan add vid {{vlan_id}} dev Bridge self &&
               /sbin/ip link add link Bridge up name Vlan{{vlan_id}} address {{gMacAddress}} type vlan id {{vlan_id}}"

/bin/echo 0 > /proc/sys/net/ipv4/conf/Vlan{{vlan_id}}/arp_evict_nocarrier
```

**Remove VLAN** (removeHostVlan):
```bash
/bin/bash -c "/sbin/ip link del Vlan{{vlan_id}} &&
               /sbin/bridge vlan del vid {{vlan_id}} dev Bridge self"
```

**Set VLAN Admin State**:
```bash
/sbin/ip link set Vlan{{vlan_id}} {{admin_status}}  # up or down
```

**Set VLAN MTU**:
```bash
/sbin/ip link set Vlan{{vlan_id}} mtu {{mtu}}
```

**Set VLAN MAC**:
```bash
/sbin/ip link set Bridge down
/sbin/ip link set Vlan{{vlan_id}} address {{mac}} && /sbin/ip link set Bridge address {{mac}}
/sbin/ip link set Bridge up
```

**Add VLAN Member**:
```bash
/bin/bash -c "/sbin/ip link set {{port_alias}} master Bridge &&
               /sbin/bridge vlan del vid 1 dev {{port_alias}} &&
               /sbin/bridge vlan add vid {{vlan_id}} dev {{port_alias}} {{tagging_mode}}"
```
- `tagging_mode`: "pvid untagged" for untagged/priority_tagged, "" for tagged

**Remove VLAN Member** (complex):
```bash
/bin/bash -c '/sbin/bridge vlan del vid {{vlan_id}} dev {{port_alias}} &&
               ( vlanShow=$(/sbin/bridge vlan show dev {{port_alias}});
               ret=$?;
               if [ $ret -eq 0 ]; then
               if (! echo "$vlanShow" | /bin/grep -q {{port_alias}})
                 || (echo "$vlanShow" | /bin/grep -q None$)
                 || (echo "$vlanShow" | /bin/grep -q {{port_alias}}$); then
               /sbin/ip link set {{port_alias}} nomaster;
               fi;
               else exit $ret; fi )'
```
**Logic**: Delete VLAN from port, then check if port has any remaining VLANs. If not, detach from bridge.

#### 3. Processing Flow

**doTask** (main entry point):
```
1. Check table name (VLAN, VLAN_MEMBER, STATE_PORT_TABLE, STATE_LAG_TABLE, etc.)
2. Dispatch to appropriate handler:
   - CFG_VLAN_TABLE → doVlanTask()
   - CFG_VLAN_MEMBER_TABLE → doVlanMemberTask()
   - STATE_PORT_TABLE → doVlanPacPortTask()
   - STATE_LAG_TABLE → doVlanPacPortTask()
   - STATE_VLAN_TABLE → doVlanPacVlanTask()
   - STATE_FDB_FLUSH_TABLE → doVlanPacFdbTask()
   - STATE_VLAN_MEMBER_TABLE → doVlanPacVlanMemberTask()
```

**doVlanTask** (VLAN configuration):
```
For each VLAN SET operation:
1. Check if VLAN MAC is ready (gMacAddress initialized)
2. Extract VLAN ID from key "Vlan1000" → 1000
3. Check if VLAN state is OK (port/LAG state)
4. Process fields:
   - "admin_status" → setHostVlanAdminState()
   - "mtu" → setHostVlanMtu()
   - "mac" → setHostVlanMac()
5. If new VLAN: addHostVlan()
6. Write to APPL_DB (m_appVlanTableProducer)
7. Track in m_vlans set
8. Handle warm restart replay

For each VLAN DEL operation:
1. Remove from warm restart replay if present
2. Call removeHostVlan()
3. Delete from APPL_DB
4. Remove from m_vlans set
```

**doVlanMemberTask** (VLAN membership):
```
For each VLAN_MEMBER SET operation:
Key format: "Vlan100|Ethernet0"
1. Parse key into VLAN and port alias
2. Check if member state is OK (port/LAG operational)
3. Extract tagging_mode field ("tagged", "untagged", "priority_tagged")
4. Call addHostVlanMember(vlan_id, port_alias, tagging_mode)
5. Write to APPL_DB (m_appVlanMemberTableProducer)
6. Track in m_PortVlanMember[port][vlan] = tagging_mode
7. Handle warm restart replay

For each VLAN_MEMBER DEL operation:
1. Remove from warm restart replay if present
2. Call removeHostVlanMember(vlan_id, port_alias)
3. Delete from APPL_DB
4. Remove from m_PortVlanMember
5. Handle "untagged_members" cleanup
```

#### 4. State Validation

**isVlanMacOk()**:
- Check if global MAC address is initialized
- Defer VLAN operations until MAC ready

**isMemberStateOk(alias)**:
- Check if port/LAG is in STATE_DB with "ok" status
- Defer member operations until port ready

**isVlanStateOk(alias)**:
- Similar to isMemberStateOk for VLAN operations

**isVlanMemberStateOk(key)**:
- Combined check for VLAN member readiness

#### 5. Warm Restart Support

**Initialization** (constructor):
```cpp
if (WarmStart::isWarmStart()) {
    // Cache all VLAN keys from CONFIG_DB
    m_cfgVlanTable.getKeys(vlanKeys);
    m_cfgVlanMemberTable.getKeys(vlanMemberKeys);

    for (auto k : vlanKeys) {
        m_vlanReplay.insert(k);
    }
    for (auto k : vlanMemberKeys) {
        m_vlanMemberReplay.insert(k);
    }

    // If no VLANs to replay, immediately mark as done
    if (m_vlanReplay.empty()) {
        replayDone = true;
        WarmStart::setWarmStartState("vlanmgrd", WarmStart::REPLAYED);
        WarmStart::setWarmStartState("vlanmgrd", WarmStart::RECONCILED);
    }

    // Skip bridge init if Bridge already exists
    if (bridge_exists()) {
        return;  // Skip initialization
    }
}
```

**Replay Processing**:
```cpp
// In doVlanTask():
if (!m_vlanReplay.empty()) {
    m_vlanReplay.erase(key);  // Remove from replay list

    if (m_vlanReplay.empty() && m_vlanMemberReplay.empty()) {
        replayDone = true;
        WarmStart::setWarmStartState("vlanmgrd", WarmStart::REPLAYED);
    }
}

// Similar in doVlanMemberTask()
```

**Reconciliation**:
- After all replay items processed, set RECONCILED state
- Allow normal operations to proceed

#### 6. Special Cases

**Untagged Members Handling**:
- Field "untagged_members" in VLAN table contains comma-separated port list
- Process each untagged member individually
- Convert to VLAN_MEMBER entries

**LAG Race Condition**:
- When adding LAG as VLAN member, PortChannel might be removed concurrently
- Retry once if LAG operation fails
- Log warning but don't fail

**MTU Validation**:
- VLAN MTU cannot exceed member port MTU
- Return false if MTU set fails (member MTU constraint)

**MAC Address Change**:
- Bridge must be brought down before MAC change
- Both Bridge and VLAN interface MAC updated together
- Bridge brought back up to refresh IPv6 link-local

---

## Rust Design

### 1. Crate Structure

```
vlanmgrd/
├── src/
│   ├── lib.rs              # Public API
│   ├── main.rs             # Daemon entry point
│   ├── vlan_mgr.rs         # VlanMgr implementation (~400 lines)
│   ├── types.rs            # VlanInfo, MemberInfo types
│   ├── tables.rs           # Table name constants
│   ├── bridge.rs           # Bridge initialization (~80 lines)
│   └── commands.rs         # Shell command builders (~100 lines)
├── tests/
│   └── integration/        # Integration tests
└── Cargo.toml
```

### 2. Type Definitions

```rust
// types.rs

/// VLAN configuration information
#[derive(Debug, Clone, Default)]
pub struct VlanInfo {
    pub vlan_id: u16,
    pub admin_status: String,  // "up" or "down"
    pub mtu: u32,
    pub mac: String,
    pub members: HashMap<String, String>,  // port -> tagging_mode
}

/// VLAN member information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlanMemberInfo {
    pub vlan_id: u16,
    pub port_alias: String,
    pub tagging_mode: TaggingMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaggingMode {
    Tagged,
    Untagged,
    PriorityTagged,
}

impl TaggingMode {
    pub fn to_bridge_cmd(&self) -> &str {
        match self {
            TaggingMode::Tagged => "",
            TaggingMode::Untagged | TaggingMode::PriorityTagged => "pvid untagged",
        }
    }
}
```

### 3. VlanMgr Structure

```rust
// vlan_mgr.rs

use sonic_cfgmgr_common::{CfgMgr, CfgMgrResult, FieldValues, Orch, WarmRestartState};
use std::collections::{HashMap, HashSet};

pub struct VlanMgr {
    /// Active VLANs
    vlans: HashSet<String>,

    /// VLAN information cache
    vlan_info: HashMap<u16, VlanInfo>,

    /// Port to VLAN membership: port -> vlan -> tagging_mode
    port_vlan_member: HashMap<String, HashMap<String, String>>,

    /// Warm restart replay lists
    vlan_replay: HashSet<String>,
    vlan_member_replay: HashSet<String>,
    replay_done: bool,

    /// Global MAC address (from config)
    global_mac: Option<String>,

    /// Mock mode for testing
    #[cfg(test)]
    mock_mode: bool,

    /// Captured commands in mock mode
    #[cfg(test)]
    captured_commands: Vec<String>,
}

impl VlanMgr {
    pub fn new() -> Self {
        Self {
            vlans: HashSet::new(),
            vlan_info: HashMap::new(),
            port_vlan_member: HashMap::new(),
            vlan_replay: HashSet::new(),
            vlan_member_replay: HashSet::new(),
            replay_done: false,
            global_mac: None,
            #[cfg(test)]
            mock_mode: false,
            #[cfg(test)]
            captured_commands: Vec::new(),
        }
    }

    /// Initialize bridge on startup
    pub async fn init_bridge(&mut self, mac_address: &str) -> CfgMgrResult<()>;

    /// Check if VLAN MAC is ready
    pub fn is_vlan_mac_ok(&self) -> bool {
        self.global_mac.is_some()
    }

    /// Add VLAN interface
    pub async fn add_host_vlan(&mut self, vlan_id: u16) -> CfgMgrResult<bool>;

    /// Remove VLAN interface
    pub async fn remove_host_vlan(&mut self, vlan_id: u16) -> CfgMgrResult<bool>;

    /// Set VLAN admin state
    pub async fn set_host_vlan_admin_state(&mut self, vlan_id: u16, admin_status: &str) -> CfgMgrResult<bool>;

    /// Set VLAN MTU
    pub async fn set_host_vlan_mtu(&mut self, vlan_id: u16, mtu: u32) -> CfgMgrResult<bool>;

    /// Set VLAN MAC address
    pub async fn set_host_vlan_mac(&mut self, vlan_id: u16, mac: &str) -> CfgMgrResult<bool>;

    /// Add VLAN member
    pub async fn add_host_vlan_member(
        &mut self,
        vlan_id: u16,
        port_alias: &str,
        tagging_mode: TaggingMode,
    ) -> CfgMgrResult<bool>;

    /// Remove VLAN member
    pub async fn remove_host_vlan_member(&mut self, vlan_id: u16, port_alias: &str) -> CfgMgrResult<bool>;

    /// Process VLAN SET operation
    pub async fn process_vlan_set(&mut self, key: &str, values: &FieldValues) -> CfgMgrResult<()>;

    /// Process VLAN DEL operation
    pub async fn process_vlan_del(&mut self, key: &str) -> CfgMgrResult<()>;

    /// Process VLAN_MEMBER SET operation
    pub async fn process_vlan_member_set(&mut self, key: &str, values: &FieldValues) -> CfgMgrResult<()>;

    /// Process VLAN_MEMBER DEL operation
    pub async fn process_vlan_member_del(&mut self, key: &str) -> CfgMgrResult<()>;
}
```

### 4. Command Builders

```rust
// commands.rs

use sonic_cfgmgr_common::shell;

pub const DOT1Q_BRIDGE_NAME: &str = "Bridge";
pub const VLAN_PREFIX: &str = "Vlan";
pub const LAG_PREFIX: &str = "PortChannel";
pub const DEFAULT_VLAN_ID: &str = "1";
pub const DEFAULT_MTU: &str = "9100";

/// Build bridge initialization command
pub fn build_init_bridge_cmd(mac_address: &str) -> String {
    format!(
        r#"/bin/bash -c "{} link del {} 2>/dev/null; \
           {} link add {} up type bridge && \
           {} link set {} mtu {} && \
           {} link set {} address {} && \
           {} vlan del vid {} dev {} self; \
           {} link del dev dummy 2>/dev/null; \
           {} link add dummy type dummy && \
           {} link set dummy master {} && \
           {} link set dummy up; \
           {} link set {} down && \
           {} link set {} up""#,
        shell::IP_CMD, DOT1Q_BRIDGE_NAME,
        shell::IP_CMD, DOT1Q_BRIDGE_NAME,
        shell::IP_CMD, DOT1Q_BRIDGE_NAME, DEFAULT_MTU,
        shell::IP_CMD, DOT1Q_BRIDGE_NAME, mac_address,
        shell::BRIDGE_CMD, DEFAULT_VLAN_ID, DOT1Q_BRIDGE_NAME,
        shell::IP_CMD,
        shell::IP_CMD,
        shell::IP_CMD, DOT1Q_BRIDGE_NAME,
        shell::IP_CMD,
        shell::IP_CMD, DOT1Q_BRIDGE_NAME,
        shell::IP_CMD, DOT1Q_BRIDGE_NAME,
    )
}

/// Enable VLAN filtering on bridge
pub fn build_vlan_filtering_cmd() -> String {
    format!(
        "{} link set {} type bridge vlan_filtering 1",
        shell::IP_CMD, DOT1Q_BRIDGE_NAME
    )
}

/// Build add VLAN command
pub fn build_add_vlan_cmd(vlan_id: u16, mac_address: &str) -> String {
    format!(
        r#"/bin/bash -c "{} vlan add vid {} dev {} self && \
           {} link add link {} up name {}{} address {} type vlan id {}""#,
        shell::BRIDGE_CMD, vlan_id, DOT1Q_BRIDGE_NAME,
        shell::IP_CMD, DOT1Q_BRIDGE_NAME, VLAN_PREFIX, vlan_id, mac_address, vlan_id
    )
}

/// Build remove VLAN command
pub fn build_remove_vlan_cmd(vlan_id: u16) -> String {
    format!(
        r#"/bin/bash -c "{} link del {}{} && \
           {} vlan del vid {} dev {} self""#,
        shell::IP_CMD, VLAN_PREFIX, vlan_id,
        shell::BRIDGE_CMD, vlan_id, DOT1Q_BRIDGE_NAME
    )
}

/// Build add VLAN member command
pub fn build_add_vlan_member_cmd(vlan_id: u16, port_alias: &str, tagging_cmd: &str) -> String {
    let port_quoted = shell::shellquote(port_alias);
    let inner = format!(
        "{} link set {} master {} && \
         {} vlan del vid {} dev {} && \
         {} vlan add vid {} dev {} {}",
        shell::IP_CMD, port_quoted, DOT1Q_BRIDGE_NAME,
        shell::BRIDGE_CMD, DEFAULT_VLAN_ID, port_quoted,
        shell::BRIDGE_CMD, vlan_id, port_quoted, tagging_cmd
    );
    format!("/bin/bash -c {}", shell::shellquote(&inner))
}

/// Build remove VLAN member command (complex with nomaster logic)
pub fn build_remove_vlan_member_cmd(vlan_id: u16, port_alias: &str) -> String {
    let port_quoted = shell::shellquote(port_alias);
    let inner = format!(
        r#"{} vlan del vid {} dev {} && \
           ( vlanShow=$({} vlan show dev {}); \
           ret=$?; \
           if [ $ret -eq 0 ]; then \
           if (! echo "$vlanShow" | {} -q {}) \
             || (echo "$vlanShow" | {} -q None$) \
             || (echo "$vlanShow" | {} -q {}$); then \
           {} link set {} nomaster; \
           fi; \
           else exit $ret; fi )"#,
        shell::BRIDGE_CMD, vlan_id, port_quoted,
        shell::BRIDGE_CMD, port_quoted,
        shell::GREP_CMD, port_quoted,
        shell::GREP_CMD,
        shell::GREP_CMD, port_quoted,
        shell::IP_CMD, port_quoted
    );
    format!("/bin/bash -c {}", shell::shellquote(&inner))
}
```

### 5. Implementation Estimates

| Component | Lines | Complexity |
|-----------|-------|------------|
| types.rs | 80 | Low |
| tables.rs | 40 | Low |
| commands.rs | 100 | Medium (bash escaping) |
| bridge.rs | 80 | Medium (init logic) |
| vlan_mgr.rs | 250 | High (main logic) |
| **Total Core** | **~550** | **Medium-High** |
| Tests | 200+ | High |
| **Grand Total** | **~750+** | |

### 6. Test Plan

**Unit Tests** (15-18 total):

| Test Category | Count | Tests |
|---------------|-------|-------|
| Bridge Init | 2 | test_init_bridge, test_vlan_filtering |
| VLAN Operations | 5 | test_add_vlan, test_remove_vlan, test_set_admin_state, test_set_mtu, test_set_mac |
| Member Operations | 4 | test_add_member_tagged, test_add_member_untagged, test_remove_member, test_remove_last_member |
| Command Building | 3 | test_build_commands, test_shellquote_safety, test_tagging_mode |
| State Tracking | 3 | test_vlan_tracking, test_member_tracking, test_port_vlan_map |
| Warm Restart | 2 | test_warm_restart_init, test_replay_complete |
| Trait Impl | 1 | test_cfgmgr_trait |

**Integration Tests** (using sonic-cfgmgr-test):
- Multi-VLAN configuration
- VLAN with multiple members
- Tagging mode combinations
- Warm restart replay

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Complex bash escaping | Medium | High | Use shellquote() consistently, comprehensive tests |
| Bridge state management | Low | High | Mock mode testing, careful state tracking |
| LAG race conditions | Medium | Medium | Retry logic, error handling |
| Warm restart complexity | Medium | High | Thorough replay testing |
| Command execution failures | Medium | Medium | Error propagation, logging |

---

## Implementation Phases

### Phase 1: Foundation (Day 1)
- ✅ Planning document (this file)
- Create vlanmgrd crate structure
- Implement types.rs and tables.rs
- Implement commands.rs with shellquote

### Phase 2: Core VLAN Operations (Day 2)
- Implement bridge.rs initialization
- Implement vlan_mgr.rs VLAN operations
- Add/remove VLAN interfaces
- Set admin state, MTU, MAC

### Phase 3: Member Operations (Day 3)
- Implement VLAN member add/remove
- Port-VLAN membership tracking
- Tagging mode handling
- LAG retry logic

### Phase 4: Testing & Refinement (Day 4)
- Write 15-18 unit tests
- Integration tests with sonic-cfgmgr-test
- Warm restart testing
- Documentation

---

## Success Criteria

| Criterion | Target | Status |
|-----------|--------|--------|
| Rust LOC | 450-500 | TBD |
| C++ reduction | 50%+ | TBD |
| Unit tests | 15+ | TBD |
| Test pass rate | 100% | TBD |
| Clippy warnings | 0 | TBD |
| Shell command safety | 100% shellquote | TBD |
| Warm restart support | Full | TBD |

---

## References

- **C++ Source**: [vlanmgr.cpp](../../../cfgmgr/vlanmgr.cpp), [vlanmgr.h](../../../cfgmgr/vlanmgr.h)
- **Test Infrastructure**: sonic-cfgmgr-test (Week 5)
- **Similar Managers**: portmgrd (simple), intfmgrd (future, similar complexity)

---

**Planning Status**: COMPLETE
**Next Step**: Begin Phase 2 implementation
**Expected Completion**: Week 6 (4 days)
