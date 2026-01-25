# neighsyncd Performance Optimizations for Linux

This document describes performance optimizations for the Rust neighsyncd daemon on Linux systems.

## NIST 800-53 Rev 5 Control Mappings

- **SC-5**: Denial of Service Protection - Optimizations prevent event queue overflow
- **AU-12**: Audit Record Generation - High-throughput logging without blocking
- **CP-10**: System Recovery - Fast warm restart reconciliation

---

## Implementation Status

All optimizations have been implemented. Use `AsyncNeighSync` for the fully optimized async mode.

| Optimization | Priority | Status | Location |
|-------------|----------|--------|----------|
| Async Netlink (epoll) | P0 | **Implemented** | `netlink.rs:AsyncNetlinkSocket` |
| Redis Pipelining | P0 | **Implemented** | `redis_adapter.rs:set_neighbors_batch()` |
| Socket Buffer Tuning | P1 | **Implemented** | `netlink.rs:tune_socket()` |
| Link-Local Cache | P1 | **Implemented** | `redis_adapter.rs:link_local_cache` |
| FxHashMap | P1 | **Implemented** | `netlink.rs:InterfaceCache` (feature flag) |
| Batch Event Processing | P2 | **Implemented** | `neigh_sync.rs:process_events_batched()` |
| Zero-Copy Parsing | P2 | **Implemented** | `netlink.rs:parse_buffer()` |
| Pre-allocated Buffer | P3 | **Implemented** | `netlink.rs:events_buffer` |

---

## 1. Async Netlink with epoll (P0 - Implemented)

**Problem**: Blocking `recv()` ties up the tokio runtime or requires busy-wait polling.

**Solution**: `AsyncNetlinkSocket` uses `AsyncFd` to integrate with tokio's event loop.

```rust
// Usage - main.rs uses AsyncNeighSync which wraps AsyncNetlinkSocket
let mut neigh_sync = AsyncNeighSync::new(host, port).await?;

// True async - yields when no data available
loop {
    let count = neigh_sync.process_events_batched().await?;
}
```

**Actual Implementation**: See `src/netlink.rs:AsyncNetlinkSocket`

**Expected Improvement**: 10-20% reduction in CPU usage under load.

---

## 2. Redis Pipelining (P0 - Implemented)

**Problem**: Each neighbor update requires a separate round-trip to Redis.

**Solution**: `set_neighbors_batch()` and `delete_neighbors_batch()` use Redis pipelines.

```rust
// Usage - called by process_events_batched()
self.redis.set_neighbors_batch(&batch_sets).await?;
self.redis.delete_neighbors_batch(&batch_deletes).await?;
```

**Actual Implementation**: See `src/redis_adapter.rs`

**Expected Improvement**: 5-10x throughput for bulk operations (warm restart).

---

## 3. Socket Buffer Tuning (P1 - Implemented)

**Problem**: Default socket buffer may overflow during neighbor table dumps.

**Solution**: `tune_socket()` sets 1MB receive buffer and enables `NETLINK_NO_ENOBUFS`.

**Actual Implementation**: See `src/netlink.rs:NetlinkSocket::tune_socket()`

**Expected Improvement**: Prevents event loss under burst load (10K+ neighbors).

---

## 4. Link-Local Configuration Cache (P1 - Implemented)

**Problem**: Each IPv6 link-local neighbor requires a CONFIG_DB lookup.

**Solution**: TTL-based cache (60 seconds) in `RedisAdapter`.

**Actual Implementation**: See `src/redis_adapter.rs:is_ipv6_link_local_enabled()`

**Expected Improvement**: Eliminates ~90% of CONFIG_DB queries for link-local checks.

---

## 5. FxHashMap for Interface Cache (P1 - Implemented)

**Problem**: `std::collections::HashMap` uses SipHash which is slower for small keys.

**Solution**: Use `FxHashMap` when `perf-fxhash` feature is enabled.

```toml
# Enable in Cargo.toml
[dependencies]
sonic-neighsyncd = { features = ["perf-fxhash"] }
```

**Actual Implementation**: See `src/netlink.rs:InterfaceCache`

**Expected Improvement**: 2-3x faster interface lookups.

---

## 6. Batch Event Processing (P2 - Implemented)

**Problem**: Processing events one-by-one with individual Redis calls.

**Solution**: `process_events_batched()` accumulates events and batches Redis operations.

**Actual Implementation**: See `src/neigh_sync.rs:NeighSync::process_events_batched()`

**Expected Improvement**: 3-5x throughput for high-volume scenarios.

---

## 7. Zero-Copy Netlink Parsing (P2 - Implemented)

**Problem**: Allocating buffers for each netlink message creates allocation pressure.

**Solution**: Parse directly from receive buffer slice.

**Actual Implementation**: See `src/netlink.rs:parse_buffer()`

**Expected Improvement**: 15-25% reduction in allocations.

---

## 8. Pre-allocated Event Buffer (P3 - Implemented)

**Problem**: Allocating a new `Vec` for each `receive_events()` call.

**Solution**: Reuse `events_buffer` with `clear()`.

**Actual Implementation**: See `src/netlink.rs:NetlinkSocket::events_buffer`

**Expected Improvement**: Reduced allocation overhead, ~5% in tight loops.

---

## Feature Flags

```toml
[features]
default = []

# Performance optimizations
perf-fxhash = ["rustc-hash"]   # FxHashMap for faster interface lookups
perf-all = ["perf-fxhash"]     # Enable all performance features
```

All other optimizations are enabled by default.

---

## Usage

### Recommended: Full Async Mode

```rust
use sonic_neighsyncd::AsyncNeighSync;

#[tokio::main]
async fn main() {
    let mut sync = AsyncNeighSync::new("127.0.0.1", 6379).await?;
    sync.request_dump()?;

    loop {
        // True async - integrates with tokio's epoll loop
        let count = sync.process_events_batched().await?;
    }
}
```

### Legacy: Blocking Mode

```rust
use sonic_neighsyncd::NeighSync;

// Still available for compatibility
let mut sync = NeighSync::new("127.0.0.1", 6379).await?;
```

---

## Benchmarking

To measure improvements:

```bash
# Build with all optimizations
cargo build --release -p sonic-neighsyncd --features perf-all

# Profile with perf
perf record -g ./target/release/neighsyncd
perf report

# Memory profiling
valgrind --tool=massif ./target/release/neighsyncd
```

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                          AsyncNeighSync                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────────┐    ┌──────────────────────────────────┐  │
│  │  AsyncNetlinkSocket  │    │         RedisAdapter              │  │
│  │                      │    │                                   │  │
│  │  ┌────────────────┐  │    │  ┌─────────────────────────────┐ │  │
│  │  │ AsyncFd<OwnedFd>│  │    │  │    set_neighbors_batch()   │ │  │
│  │  │   (epoll)       │  │    │  │    delete_neighbors_batch()│ │  │
│  │  └────────────────┘  │    │  │    (Redis pipelining)       │ │  │
│  │                      │    │  └─────────────────────────────┘ │  │
│  │  ┌────────────────┐  │    │                                   │  │
│  │  │ NetlinkSocket  │  │    │  ┌─────────────────────────────┐ │  │
│  │  │ - 1MB buffer   │  │    │  │    link_local_cache         │ │  │
│  │  │ - NO_ENOBUFS   │  │    │  │    (60s TTL)                │ │  │
│  │  │ - events_buffer│  │    │  └─────────────────────────────┘ │  │
│  │  └────────────────┘  │    │                                   │  │
│  │                      │    │                                   │  │
│  │  ┌────────────────┐  │    │                                   │  │
│  │  │InterfaceCache  │  │    │                                   │  │
│  │  │ (FxHashMap*)   │  │    │                                   │  │
│  │  └────────────────┘  │    │                                   │  │
│  └──────────────────────┘    └──────────────────────────────────┘  │
│                                                                      │
│  * FxHashMap when perf-fxhash feature enabled                       │
└─────────────────────────────────────────────────────────────────────┘
```
