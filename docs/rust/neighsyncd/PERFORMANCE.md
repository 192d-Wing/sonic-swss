# neighsyncd Performance Optimizations for Linux

This document describes performance optimizations for the Rust neighsyncd daemon on Linux systems.

## NIST 800-53 Rev 5 Control Mappings

- **SC-5**: Denial of Service Protection - Optimizations prevent event queue overflow
- **AU-12**: Audit Record Generation - High-throughput logging without blocking
- **CP-10**: System Recovery - Fast warm restart reconciliation

---

## 1. Async Netlink with epoll (High Impact)

**Problem**: Current implementation uses blocking `recv()` which ties up the tokio runtime.

**Solution**: Use `AsyncFd` to integrate netlink socket with tokio's event loop.

```rust
use tokio::io::unix::AsyncFd;
use std::os::fd::AsRawFd;

pub struct AsyncNetlinkSocket {
    inner: AsyncFd<Socket>,
    buffer: Vec<u8>,
}

impl AsyncNetlinkSocket {
    pub async fn recv_events(&mut self) -> Result<Vec<NeighborEvent>> {
        loop {
            let mut guard = self.inner.readable().await?;

            match guard.try_io(|inner| {
                inner.get_ref().recv(&mut self.buffer, libc::MSG_DONTWAIT)
            }) {
                Ok(Ok(len)) => return self.parse_events(len),
                Ok(Err(e)) => return Err(e.into()),
                Err(_would_block) => continue,
            }
        }
    }
}
```

**Expected Improvement**: 10-20% reduction in CPU usage under load.

---

## 2. Redis Pipelining (High Impact)

**Problem**: Each neighbor update requires a separate round-trip to Redis.

**Solution**: Batch multiple operations into a single pipeline.

```rust
use redis::pipe;

impl RedisAdapter {
    /// Batch set multiple neighbor entries
    /// NIST: AU-12 - Efficient bulk audit record generation
    pub async fn set_neighbors_batch(&mut self, entries: &[NeighborEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut pipe = redis::pipe();
        pipe.atomic();  // Execute as transaction

        for entry in entries {
            let key = format!("{}:{}", APP_NEIGH_TABLE_NAME, entry.redis_key());
            pipe.hset_multiple::<_, _, _, ()>(
                &key,
                &[
                    ("neigh", entry.mac.to_string()),
                    ("family", entry.family_str().to_string()),
                ],
            );
        }

        pipe.query_async(&mut self.appl_db).await?;
        Ok(())
    }

    /// Batch delete multiple neighbor entries
    pub async fn delete_neighbors_batch(&mut self, entries: &[NeighborEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut pipe = redis::pipe();
        for entry in entries {
            let key = format!("{}:{}", APP_NEIGH_TABLE_NAME, entry.redis_key());
            pipe.del::<_, ()>(&key);
        }

        pipe.query_async(&mut self.appl_db).await?;
        Ok(())
    }
}
```

**Expected Improvement**: 5-10x throughput improvement for bulk operations (warm restart).

---

## 3. Zero-Copy Netlink Parsing (Medium Impact)

**Problem**: Allocating buffers for each netlink message creates GC pressure.

**Solution**: Use stack allocation for small messages, reuse heap buffers.

```rust
/// Small message threshold - most neighbor messages are < 256 bytes
const SMALL_MSG_THRESHOLD: usize = 256;

pub fn parse_message_zerocopy(buffer: &[u8]) -> Result<NeighborEntry> {
    // Parse directly from buffer slice without copying
    let msg = NetlinkMessage::<RouteNetlinkMessage>::deserialize(buffer)?;

    // Extract fields by reference where possible
    // ...
}
```

**Expected Improvement**: 15-25% reduction in allocations.

---

## 4. FxHashMap for Interface Cache (Medium Impact)

**Problem**: `std::collections::HashMap` uses SipHash which is slower for small keys.

**Solution**: Use `FxHashMap` optimized for small integer keys.

```toml
# Cargo.toml
[target.'cfg(target_os = "linux")'.dependencies]
rustc-hash = "2.0"
```

```rust
use rustc_hash::FxHashMap;

pub struct InterfaceCache {
    cache: FxHashMap<u32, String>,
}
```

**Expected Improvement**: 2-3x faster interface lookups.

---

## 5. Pre-allocated Event Buffer (Low Impact)

**Problem**: Allocating a new `Vec` for each `receive_events()` call.

**Solution**: Reuse buffer with `clear()`.

```rust
pub struct NetlinkSocket {
    socket: Socket,
    recv_buffer: Vec<u8>,
    events_buffer: Vec<(NeighborMessageType, NeighborEntry)>,
}

impl NetlinkSocket {
    pub fn receive_events(&mut self) -> Result<&[(NeighborMessageType, NeighborEntry)]> {
        self.events_buffer.clear();

        let len = self.socket.recv(&mut self.recv_buffer, 0)?;
        // ... parse into self.events_buffer

        Ok(&self.events_buffer)
    }
}
```

**Expected Improvement**: Reduced allocation overhead, ~5% in tight loops.

---

## 6. Socket Buffer Tuning (Medium Impact)

**Problem**: Default socket buffer may overflow during neighbor table dumps.

**Solution**: Increase `SO_RCVBUF` and enable `NETLINK_NO_ENOBUFS`.

```rust
use nix::sys::socket::{setsockopt, sockopt::RcvBuf};
use std::os::fd::AsRawFd;

impl NetlinkSocket {
    pub fn tune_socket(&self) -> Result<()> {
        let fd = self.socket.as_raw_fd();

        // Increase receive buffer to 1MB
        // NIST: SC-5 - Prevent buffer overflow DoS
        setsockopt(&fd, RcvBuf, &(1024 * 1024))?;

        // Prevent ENOBUFS under load
        unsafe {
            let enable: libc::c_int = 1;
            libc::setsockopt(
                fd,
                libc::SOL_NETLINK,
                libc::NETLINK_NO_ENOBUFS,
                &enable as *const _ as *const libc::c_void,
                std::mem::size_of::<libc::c_int>() as libc::socklen_t,
            );
        }

        Ok(())
    }
}
```

**Expected Improvement**: Prevents event loss under burst load (10K+ neighbors).

---

## 7. Link-Local Configuration Cache (Medium Impact)

**Problem**: Each IPv6 link-local neighbor requires a CONFIG_DB lookup.

**Solution**: Cache interface link-local settings with TTL.

```rust
use std::time::{Duration, Instant};

const LINK_LOCAL_CACHE_TTL: Duration = Duration::from_secs(60);

pub struct LinkLocalCache {
    cache: HashMap<String, (bool, Instant)>,
}

impl LinkLocalCache {
    pub fn get(&self, interface: &str) -> Option<bool> {
        self.cache.get(interface).and_then(|(enabled, timestamp)| {
            if timestamp.elapsed() < LINK_LOCAL_CACHE_TTL {
                Some(*enabled)
            } else {
                None
            }
        })
    }

    pub fn set(&mut self, interface: String, enabled: bool) {
        self.cache.insert(interface, (enabled, Instant::now()));
    }
}
```

**Expected Improvement**: Eliminates ~90% of CONFIG_DB queries for link-local checks.

---

## 8. Batch Event Processing (Medium Impact)

**Problem**: Processing events one-by-one with individual Redis calls.

**Solution**: Accumulate events and process in batches.

```rust
const BATCH_SIZE: usize = 100;
const BATCH_TIMEOUT: Duration = Duration::from_millis(10);

impl NeighSync {
    pub async fn process_events_batched(&mut self) -> Result<usize> {
        let mut batch_sets: Vec<NeighborEntry> = Vec::with_capacity(BATCH_SIZE);
        let mut batch_deletes: Vec<NeighborEntry> = Vec::with_capacity(BATCH_SIZE);

        let events = self.netlink.receive_events()?;

        for (msg_type, entry) in events {
            if !self.should_process_entry(&entry).await? {
                continue;
            }

            if self.should_delete(&msg_type, &entry) {
                batch_deletes.push(entry);
            } else {
                batch_sets.push(entry);
            }
        }

        // Batch Redis operations
        self.redis.set_neighbors_batch(&batch_sets).await?;
        self.redis.delete_neighbors_batch(&batch_deletes).await?;

        Ok(batch_sets.len() + batch_deletes.len())
    }
}
```

**Expected Improvement**: 3-5x throughput for high-volume scenarios.

---

## Implementation Priority

| Optimization | Impact | Complexity | Priority |
|-------------|--------|------------|----------|
| Async Netlink (epoll) | High | Medium | P0 |
| Redis Pipelining | High | Low | P0 |
| Socket Buffer Tuning | Medium | Low | P1 |
| Link-Local Cache | Medium | Low | P1 |
| FxHashMap | Medium | Low | P1 |
| Batch Event Processing | Medium | Medium | P2 |
| Zero-Copy Parsing | Medium | High | P2 |
| Pre-allocated Buffer | Low | Low | P3 |

---

## Benchmarking

To measure improvements, use the following test scenarios:

```bash
# Warm restart with 10K neighbors
time cargo run --release -- --benchmark warm-restart --neighbors 10000

# Sustained event rate
cargo run --release -- --benchmark sustained --rate 1000 --duration 60

# Burst handling
cargo run --release -- --benchmark burst --events 5000
```

---

## Feature Flags

Performance optimizations can be enabled via feature flags:

```toml
[features]
default = []
perf-async-netlink = []      # Async netlink with epoll
perf-redis-pipeline = []     # Redis pipelining
perf-fxhash = ["rustc-hash"] # FxHashMap for caches
perf-all = ["perf-async-netlink", "perf-redis-pipeline", "perf-fxhash"]
```
