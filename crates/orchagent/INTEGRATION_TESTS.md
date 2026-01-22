# Integration Testing Architecture

## Overview

This document describes the integration testing infrastructure for sonic-orchagent modules. Integration tests verify that orchestration modules interact correctly with the SAI (Switch Abstraction Interface) layer.

## MockSai Implementation

The `MockSai` struct simulates SAI behavior without requiring actual hardware or the SAI library:

```rust
pub struct MockSai {
    objects: Arc<Mutex<Vec<SaiObject>>>,
    next_oid: Arc<Mutex<u64>>,
}
```

### Key Features

1. **Object Tracking**: Maintains a list of all created SAI objects
2. **OID Generation**: Automatically assigns unique object IDs
3. **Thread-Safe**: Uses `Arc<Mutex<>>` for concurrent access
4. **Object Types**: Supports all SAI object types (Port, Route, Neighbor, Tunnel, Buffer, QoS, etc.)

### MockSai API

- `create_object(type, attributes)` - Creates a SAI object and returns its OID
- `remove_object(oid)` - Removes a SAI object by OID
- `get_object(oid)` - Retrieves a SAI object by OID
- `count_objects(type)` - Counts objects of a specific type
- `clear()` - Removes all objects (for test cleanup)

## Integration Test Pattern

Each orchestration module has corresponding integration tests following this pattern:

### 1. Helper Function

Create test entries with SAI objects:

```rust
fn create_neighbor_with_sai(
    ip: &str,
    interface: &str,
    mac: &str,
    sai: &MockSai
) -> (NeighborEntry, u64) {
    // Parse parameters
    let ip_addr: IpAddr = ip.parse().unwrap();
    let mac_addr = MacAddress::from_str(mac).unwrap();
    let key = NeighborKey::new(interface.to_string(), ip_addr);

    // Create orchestration entry
    let mut entry = NeighborEntry::new(key, mac_addr);

    // Create SAI object
    let oid = sai.create_object(
        SaiObjectType::Neighbor,
        vec![
            ("ip".to_string(), ip.to_string()),
            ("interface".to_string(), interface.to_string()),
            ("mac".to_string(), mac.to_string()),
        ]
    ).unwrap();

    // Link orchestration entry to SAI object
    entry.neigh_oid = oid;
    (entry, oid)
}
```

### 2. Integration Tests

Test end-to-end workflows:

```rust
#[test]
fn test_neigh_orch_add_creates_sai_object() {
    let sai = MockSai::new();
    let mut orch = NeighOrch::new(NeighOrchConfig::default());

    let (neighbor, oid) = create_neighbor_with_sai(
        "10.0.0.1", "Ethernet0", "00:11:22:33:44:55", &sai
    );
    orch.add_neighbor(neighbor).unwrap();

    // Verify orchestration state
    assert_eq!(orch.neighbor_count(), 1);

    // Verify SAI object created
    assert_eq!(sai.count_objects(SaiObjectType::Neighbor), 1);
    let sai_obj = sai.get_object(oid).unwrap();
    assert_eq!(sai_obj.object_type, SaiObjectType::Neighbor);
}
```

## Test Coverage

### NeighOrch (4 integration tests)

1. `test_neigh_orch_add_creates_sai_object` - Verify neighbor creation creates SAI object
2. `test_neigh_orch_remove_deletes_sai_object` - Verify neighbor removal deletes SAI object
3. `test_neigh_orch_multiple_neighbors` - Verify multiple neighbors with correct IPv4/IPv6 stats
4. Added to [integration_test.rs:194-272](integration_test.rs#L194-L272)

### BufferOrch (4 integration tests)

1. `test_buffer_orch_add_pool_creates_sai_object` - Verify pool creation creates SAI object
2. `test_buffer_orch_add_profile_with_pool` - Verify profile depends on pool existence
3. `test_buffer_orch_ref_counting_prevents_removal` - Verify ref counting prevents premature removal
4. `test_buffer_orch_remove_after_ref_count_zero` - Verify removal succeeds when ref count reaches zero
5. Added to [integration_test.rs:275-398](integration_test.rs#L275-L398)

### VxlanOrch (4 integration tests)

1. `test_vxlan_orch_add_tunnel_creates_sai_object` - Verify tunnel creation creates SAI object
2. `test_vxlan_orch_remove_tunnel_deletes_sai_object` - Verify tunnel removal deletes SAI object
3. `test_vxlan_orch_multiple_tunnels` - Verify multiple tunnels managed correctly
4. `test_vxlan_orch_vrf_and_vlan_maps` - Verify VRF/VLAN map creation
5. Added to [integration_test.rs:401-489](integration_test.rs#L401-L489)

## Test Execution

Integration tests are located in `tests/integration_test.rs` and can be run with:

```bash
cargo test --test integration_test
```

To run a specific test module:

```bash
cargo test --test integration_test neigh_orch_tests
cargo test --test integration_test buffer_orch_tests
cargo test --test integration_test vxlan_orch_tests
```

## Benefits

1. **Hardware Independence**: Tests run without actual network hardware
2. **SAI Verification**: Confirms orchestration modules create correct SAI objects
3. **End-to-End Validation**: Tests full workflow from config to SAI
4. **Fast Execution**: MockSai operations are in-memory and instant
5. **Deterministic**: No hardware timing or state issues
6. **CI/CD Ready**: Can run in any environment

## Future Work

Add integration tests for remaining orchestration modules:

- QosOrch (DSCP maps, schedulers, WRED profiles)
- Srv6Orch (local SIDs, SID lists)
- MacsecOrch (ports, SCs, SAs)
- VnetOrch (VNETs, routes, tunnels)
- NatOrch (SNAT/DNAT entries, pools)
- RouteOrch (routes, next-hop groups)
- FdbOrch (MAC learning, forwarding entries)

## Architecture Diagram

```
┌─────────────────────────────────────────────────────┐
│                Integration Test                      │
├─────────────────────────────────────────────────────┤
│                                                       │
│  ┌──────────────┐           ┌───────────────┐       │
│  │  NeighOrch   │◄─────────►│   MockSai     │       │
│  │  BufferOrch  │           │               │       │
│  │  VxlanOrch   │           │ • Tracks OIDs │       │
│  │     ...      │           │ • Simulates   │       │
│  └──────────────┘           │   SAI API     │       │
│         │                   └───────────────┘       │
│         │                           │               │
│         └───────────────────────────┘               │
│           Verifies object creation                  │
│           and state synchronization                 │
└─────────────────────────────────────────────────────┘
```

## Key Takeaways

1. MockSai provides lightweight SAI simulation
2. Integration tests verify orch ↔ SAI interaction
3. Pattern is consistent across all modules
4. Tests are fast, deterministic, and CI-friendly
5. Foundation for comprehensive E2E testing
