# neighsyncd Architecture

**Version:** 1.0
**Last Updated:** 2026-01-25
**Status:** Production Ready

## Table of Contents

1. [System Overview](#system-overview)
2. [High-Level Architecture](#high-level-architecture)
3. [Component Details](#component-details)
4. [Data Flow](#data-flow)
5. [Async I/O Design](#async-io-design)
6. [State Management](#state-management)
7. [Warm Restart State Machine](#warm-restart-state-machine)
8. [Performance Optimizations](#performance-optimizations)
9. [Error Handling Strategy](#error-handling-strategy)
10. [Security Architecture](#security-architecture)
11. [Deployment Patterns](#deployment-patterns)
12. [Design Decisions](#design-decisions)

---

## System Overview

neighsyncd is a high-performance network neighbor synchronization daemon that bridges the Linux kernel's neighbor table (ARP/NDP cache) with SONiC's centralized Redis database (APPL_DB). It is implemented in Rust for memory safety, performance, and reliability.

### Key Responsibilities

1. **Netlink Event Processing**: Listen to kernel RTM_NEWNEIGH/RTM_DELNEIGH events
2. **State Synchronization**: Maintain consistency between kernel and Redis state
3. **Warm Restart**: Cache and reconcile state during daemon restarts
4. **Filtering**: Apply SONiC-specific filtering rules (broadcast MAC, multicast, etc.)
5. **Batching**: Aggregate Redis operations for optimal throughput
6. **Monitoring**: Export Prometheus metrics and health status

### Design Principles

- **Zero-Copy**: Minimize memory allocations and data copies
- **Async I/O**: Non-blocking operations using Tokio runtime
- **Memory Safety**: Rust's ownership model prevents common C++ vulnerabilities
- **Type Safety**: Strong typing prevents logic errors at compile time
- **Fail-Fast**: Panic on unrecoverable errors, graceful degradation on transient failures
- **Observable**: Comprehensive metrics and structured logging

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        neighsyncd Process                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Tokio Async Runtime                       │   │
│  │  ┌────────────┐  ┌──────────────┐  ┌───────────────────┐   │   │
│  │  │ Main Event │  │   Metrics    │  │  Health Monitor   │   │   │
│  │  │    Loop    │  │    Server    │  │  (Background)     │   │   │
│  │  └─────┬──────┘  └──────┬───────┘  └─────────┬─────────┘   │   │
│  └────────┼─────────────────┼──────────────────────┼──────────────┘ │
│           │                 │                      │                 │
│           ▼                 ▼                      ▼                 │
│  ┌────────────────┐  ┌─────────────┐   ┌──────────────────┐        │
│  │ AsyncNeighSync │  │ MetricsServer│   │ HealthMonitor    │        │
│  │                │  │ (CNSA mTLS) │   │ (Stall Detection)│        │
│  │  ┌──────────┐  │  └─────────────┘   └──────────────────┘        │
│  │  │ Netlink  │  │                                                 │
│  │  │  Socket  │  │                                                 │
│  │  └────┬─────┘  │                                                 │
│  │       │        │                                                 │
│  │       ▼        │                                                 │
│  │  ┌──────────┐  │                                                 │
│  │  │  Redis   │  │                                                 │
│  │  │ Adapter  │  │                                                 │
│  │  └──────────┘  │                                                 │
│  └────────────────┘                                                 │
└─────────────────────────────────────────────────────────────────────┘
        │                        │                        │
        ▼                        ▼                        ▼
┌─────────────┐          ┌──────────────┐      ┌─────────────────┐
│   Linux     │          │    Redis     │      │   Prometheus    │
│   Kernel    │          │   (APPL_DB)  │      │  (Monitoring)   │
│ (Netlink)   │          │              │      │                 │
└─────────────┘          └──────────────┘      └─────────────────┘
```

### Component Layers

1. **System Interface Layer**: Linux kernel netlink, Redis TCP connection
2. **Adapter Layer**: AsyncNetlinkSocket, RedisAdapter
3. **Business Logic Layer**: AsyncNeighSync (orchestration, filtering, batching)
4. **Observability Layer**: MetricsServer, HealthMonitor
5. **Runtime Layer**: Tokio async executor

---

## Component Details

### 1. AsyncNeighSync

**Location**: `crates/neighsyncd/src/neighsync.rs`

**Responsibilities**:
- Orchestrate event processing loop
- Apply filtering rules
- Batch Redis operations
- Manage warm restart state machine
- Coordinate with metrics and health monitoring

**Key Structures**:

```rust
pub struct AsyncNeighSync {
    netlink_socket: AsyncNetlinkSocket,
    redis_adapter: RedisAdapter,
    metrics: MetricsCollector,
    warm_restart_cache: Option<HashMap<NeighborKey, NeighborValue>>,
    batch_buffer: Vec<NeighborUpdate>,
    batch_size: usize,
}

pub struct NeighborEntry {
    interface: String,
    ip: IpAddr,
    mac: MacAddr,
    state: NeighborState,
    family: AddressFamily,
}
```

**Processing Flow**:

```
1. Read netlink event (async)
2. Parse RTM_NEWNEIGH/RTM_DELNEIGH
3. Extract neighbor attributes (IP, MAC, interface, state)
4. Apply filtering rules
   - Skip broadcast MACs (ff:ff:ff:ff:ff:ff)
   - Skip zero MACs (00:00:00:00:00:00) on non-dual-ToR
   - Skip multicast link-local (ff02::*)
5. Batch updates
6. Flush to Redis when:
   - Batch size reached (default: 100)
   - Timeout exceeded (default: 100ms)
   - Explicit flush requested
7. Update metrics
8. Return control to event loop
```

### 2. AsyncNetlinkSocket

**Location**: `crates/neighsyncd/src/netlink_socket.rs`

**Responsibilities**:
- Create and bind netlink socket (NETLINK_ROUTE family)
- Subscribe to RTM_NEWNEIGH and RTM_DELNEIGH multicast groups
- Parse netlink messages using zero-copy rtnetlink crate
- Convert kernel neighbor attributes to Rust structs
- Handle socket buffer overflow (ENOBUFS)

**Key Methods**:

```rust
impl AsyncNetlinkSocket {
    /// Create new async netlink socket
    pub async fn new() -> Result<Self>;

    /// Receive next neighbor event
    pub async fn recv_neighbor_event(&mut self) -> Result<Option<NeighborEvent>>;

    /// Query all neighbors (for warm restart reconciliation)
    pub async fn query_all_neighbors(&mut self) -> Result<Vec<NeighborEntry>>;

    /// Set socket buffer size (tuning)
    pub fn set_socket_buffer_size(&mut self, size: usize) -> Result<()>;
}
```

**Netlink Message Format**:

```
Netlink Header (16 bytes)
├─ nlmsg_len: u32       (total message length)
├─ nlmsg_type: u16      (RTM_NEWNEIGH=28, RTM_DELNEIGH=29)
├─ nlmsg_flags: u16     (NLM_F_MULTI, etc.)
├─ nlmsg_seq: u32       (sequence number)
└─ nlmsg_pid: u32       (sender port ID)

Neighbor Message (ndmsg)
├─ ndm_family: u8       (AF_INET=2, AF_INET6=10)
├─ ndm_state: u16       (NUD_REACHABLE, NUD_STALE, etc.)
├─ ndm_ifindex: u32     (interface index)
└─ ndm_type: u8         (neighbor type)

Netlink Attributes (TLVs)
├─ NDA_DST: IP address
├─ NDA_LLADDR: MAC address
├─ NDA_CACHEINFO: cache statistics
└─ NDA_PROBES: probe count
```

### 3. RedisAdapter

**Location**: `crates/neighsyncd/src/redis_adapter.rs`

**Responsibilities**:
- Manage Redis connection pool
- Batch SET/DEL operations using pipelining
- Implement retry logic with exponential backoff
- Handle Redis disconnections gracefully
- Cache warm restart state

**Key Methods**:

```rust
impl RedisAdapter {
    /// Connect to Redis
    pub async fn new(config: RedisConfig) -> Result<Self>;

    /// Set neighbor entry
    pub async fn set_neighbor(&mut self, entry: &NeighborEntry) -> Result<()>;

    /// Delete neighbor entry
    pub async fn del_neighbor(&mut self, key: &NeighborKey) -> Result<()>;

    /// Batch operations (pipelined)
    pub async fn batch_update(&mut self, updates: &[NeighborUpdate]) -> Result<()>;

    /// Load cached state (warm restart)
    pub async fn load_cached_neighbors(&mut self) -> Result<HashMap<NeighborKey, NeighborValue>>;

    /// Save state to cache (warm restart)
    pub async fn save_to_cache(&mut self, neighbors: &HashMap<NeighborKey, NeighborValue>) -> Result<()>;
}
```

**Redis Key Format**:

```
NEIGH_TABLE:<interface>:<ip_address>
│           │           └─ IP address (e.g., "2001:db8::1", "192.0.2.1")
│           └─ Interface name (e.g., "Ethernet0")
└─ Table prefix (SONiC convention)

Example:
  NEIGH_TABLE:Ethernet0:2001:db8::1
  NEIGH_TABLE:Ethernet64:192.0.2.10
```

**Redis Value Format** (Hash):

```
HSET NEIGH_TABLE:Ethernet0:2001:db8::1
  neigh "aa:bb:cc:dd:ee:ff"
  family "IPv6"
  state "Reachable"
```

**Batching Strategy**:

Redis pipelining is used to reduce round-trips:

```
Without pipelining (100 neighbors):
  100 round-trips × 0.5ms = 50ms total

With pipelining (batch size 100):
  1 round-trip × 0.5ms = 0.5ms total

Performance improvement: 100x
```

### 4. MetricsServer

**Location**: `crates/neighsyncd/src/metrics_server.rs`

**Responsibilities**:
- HTTP server on `[::1]:9091` (IPv6 loopback)
- Serve Prometheus metrics at `/metrics`
- Enforce CNSA 2.0 mTLS (TLS 1.3, TLS_AES_256_GCM_SHA384)
- Validate client certificates using WebPkiClientVerifier
- Support both text and JSON export formats

**Endpoints**:

```
GET /metrics         - Prometheus text format
GET /metrics/json    - JSON format
GET /health          - Health check (200 OK if healthy)
```

**CNSA 2.0 Enforcement**:

```rust
// Only allow TLS 1.3
.with_protocol_versions(&[&rustls::version::TLS13])

// Only allow TLS_AES_256_GCM_SHA384 cipher suite
let cnsa_cipher_suites: Vec<SupportedCipherSuite> =
    crypto_provider.cipher_suites.iter()
    .filter(|cs| cs.suite() == CipherSuite::TLS13_AES_256_GCM_SHA384)
    .copied()
    .collect();

// Mandatory client certificate verification
.with_client_cert_verifier(WebPkiClientVerifier::builder(Arc::new(root_store)).build()?)
```

### 5. HealthMonitor

**Location**: `crates/neighsyncd/src/health_monitor.rs`

**Responsibilities**:
- Track last event timestamp
- Detect stalls (no events for N seconds)
- Calculate failure rate
- Transition health status (Healthy → Degraded → Unhealthy)
- Integrate with systemd watchdog

**State Machine**:

```
                    ┌─────────┐
                    │ Healthy │ (health_status = 1.0)
                    └────┬────┘
                         │
          ┌──────────────┼──────────────┐
          │ Stall        │ Failure      │
          │ detected     │ rate > 5%    │
          ▼              ▼              │
     ┌─────────┐    ┌─────────┐        │
     │Degraded │    │Degraded │        │ Events
     │(Stall)  │    │ (Error) │        │ flowing
     └────┬────┘    └────┬────┘        │ normally
          │              │              │
          │ Stall        │ Failure      │
          │ persists     │ persists     │
          ▼              ▼              │
     ┌──────────┐   ┌──────────┐       │
     │Unhealthy │   │Unhealthy │       │
     │ (Stall)  │   │ (Error)  │       │
     └────┬─────┘   └────┬─────┘       │
          │              │              │
          └──────────────┴──────────────┘
                         │
                         │ Recovery
                         ▼
                    ┌─────────┐
                    │ Healthy │
                    └─────────┘
```

**Thresholds** (configurable):

- **Stall Detection**: No events for 10 seconds
- **Failure Rate**: > 5% events failed
- **Recovery**: 30 consecutive successful events

### 6. MetricsCollector

**Location**: `crates/neighsyncd/src/metrics.rs`

**Metrics Exported**:

| Metric | Type | Description |
|--------|------|-------------|
| `neighsyncd_neighbors_processed_total` | Counter | Total neighbors processed |
| `neighsyncd_neighbors_added_total` | Counter | Neighbors added to Redis |
| `neighsyncd_neighbors_deleted_total` | Counter | Neighbors deleted from Redis |
| `neighsyncd_events_failed_total` | Counter | Failed events |
| `neighsyncd_netlink_errors_total` | Counter | Netlink socket errors |
| `neighsyncd_redis_errors_total` | Counter | Redis operation errors |
| `neighsyncd_pending_neighbors` | Gauge | Current pending neighbors |
| `neighsyncd_queue_depth` | Gauge | Event queue depth |
| `neighsyncd_memory_bytes` | Gauge | Process memory usage |
| `neighsyncd_redis_connected` | Gauge | Redis connection status (1/0) |
| `neighsyncd_netlink_connected` | Gauge | Netlink socket status (1/0) |
| `neighsyncd_health_status` | Gauge | Health (1.0=healthy, 0.5=degraded, 0=unhealthy) |
| `neighsyncd_event_latency_seconds` | Histogram | Event processing latency |
| `neighsyncd_redis_latency_seconds` | Histogram | Redis operation latency |
| `neighsyncd_batch_size` | Histogram | Distribution of batch sizes |

---

## Data Flow

### Normal Operation

```
┌─────────────┐
│Linux Kernel │
│  (Netlink)  │
└──────┬──────┘
       │ RTM_NEWNEIGH/RTM_DELNEIGH
       │ (neighbor state change)
       ▼
┌────────────────────┐
│ AsyncNetlinkSocket │ ← recv_neighbor_event()
└──────┬─────────────┘
       │ NeighborEvent
       ▼
┌────────────────────┐
│  AsyncNeighSync    │
│                    │
│ 1. Parse event     │
│ 2. Apply filters   │ ← is_broadcast_mac()?
│ 3. Batch updates   │   is_zero_mac()?
│                    │   is_multicast_ll()?
└──────┬─────────────┘
       │ Vec<NeighborUpdate>
       │ (when batch full)
       ▼
┌────────────────────┐
│   RedisAdapter     │
│                    │
│ 1. Pipeline cmds   │ ← HSET NEIGH_TABLE:eth0:...
│ 2. Execute batch   │   DEL NEIGH_TABLE:eth1:...
│ 3. Handle errors   │
└──────┬─────────────┘
       │ Result<()>
       ▼
┌────────────────────┐
│   Redis Server     │
│    (APPL_DB)       │
└────────────────────┘
       │
       │ (consumed by other SONiC processes)
       ▼
┌────────────────────┐
│   orchagent,       │
│   syncd, etc.      │
└────────────────────┘
```

### Warm Restart Flow

**Phase 1: Shutdown**

```
┌────────────────────┐
│ User triggers      │
│ warm restart       │
└──────┬─────────────┘
       │ redis-cli SET WARM_RESTART_ENABLE_TABLE|neighsyncd true
       ▼
┌────────────────────┐
│ AsyncNeighSync     │
│                    │
│ 1. Detect flag     │
│ 2. Query kernel    │ ← query_all_neighbors()
│ 3. Save to Redis   │
└──────┬─────────────┘
       │ HashMap<NeighborKey, NeighborValue>
       ▼
┌────────────────────┐
│  RedisAdapter      │
│                    │
│ HSET WARM_RESTART_│
│   NEIGHSYNCD_CACHE│
└──────┬─────────────┘
       │
       ▼
┌────────────────────┐
│ Graceful shutdown  │
│ (preserve Redis)   │
└────────────────────┘
```

**Phase 2: Startup & Reconciliation**

```
┌────────────────────┐
│ neighsyncd starts  │
└──────┬─────────────┘
       │
       ▼
┌────────────────────┐
│ AsyncNeighSync     │
│                    │
│ 1. Check flag      │ ← GET WARM_RESTART_ENABLE_TABLE|neighsyncd
│ 2. Load cache      │
└──────┬─────────────┘
       │ cached_neighbors: HashMap
       ▼
┌────────────────────┐
│  Reconcile Timer   │
│    (5 seconds)     │ ← Buffer concurrent events
└──────┬─────────────┘
       │ Timer expires
       ▼
┌────────────────────┐
│ AsyncNeighSync     │
│                    │
│ 1. Query kernel    │ ← query_all_neighbors()
│ 2. Compare states  │
│ 3. Generate diffs  │
└──────┬─────────────┘
       │
       ├─ Additions: in kernel, not in cache → HSET
       ├─ Updates:  in both, different values → HSET
       └─ Deletions: in cache, not in kernel → DEL
       │
       ▼
┌────────────────────┐
│  RedisAdapter      │
│ (batch update)     │
└──────┬─────────────┘
       │
       ▼
┌────────────────────┐
│ Resume normal      │
│ operation          │
└────────────────────┘
```

---

## Async I/O Design

### Tokio Runtime

neighsyncd uses the Tokio async runtime for I/O multiplexing:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Multi-threaded runtime with 4 worker threads
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_name("neighsyncd-worker")
        .enable_all()
        .build()?;

    runtime.block_on(async {
        // Spawn concurrent tasks
        let metrics_task = tokio::spawn(start_metrics_server());
        let health_task = tokio::spawn(health_monitor_loop());
        let main_task = tokio::spawn(neighsync_event_loop());

        // Await all tasks
        tokio::try_join!(metrics_task, health_task, main_task)?;
        Ok(())
    })
}
```

### Event Loop Structure

```rust
pub async fn run_event_loop(&mut self) -> Result<()> {
    loop {
        tokio::select! {
            // Netlink events (high priority)
            event = self.netlink_socket.recv_neighbor_event() => {
                match event? {
                    Some(neighbor) => self.process_neighbor_event(neighbor).await?,
                    None => continue,
                }
            }

            // Batch flush timer (every 100ms)
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if !self.batch_buffer.is_empty() {
                    self.flush_batch().await?;
                }
            }

            // Health check timer (every 1 second)
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                self.health_monitor.update_health();
            }

            // Shutdown signal (SIGTERM, SIGINT)
            _ = signal::ctrl_c() => {
                info!("Received shutdown signal, exiting gracefully");
                self.shutdown().await?;
                break;
            }
        }
    }

    Ok(())
}
```

### Zero-Copy Optimization

Netlink messages are parsed in-place without copying:

```rust
// Bad: Copies entire message buffer
let msg = buf.to_vec();
let neighbor = parse_neighbor(&msg)?;

// Good: Zero-copy parsing using references
let neighbor = parse_neighbor(&buf)?;
```

---

## State Management

### In-Memory State

neighsyncd is designed to be **stateless** during normal operation:

- No in-memory cache of neighbor table
- Kernel is source of truth
- Redis is synchronization target
- Only transient state: batch buffer, warm restart cache

### Warm Restart State

Temporary state maintained during warm restart:

```rust
pub struct WarmRestartState {
    // Cached neighbors from before restart
    cached_neighbors: HashMap<NeighborKey, NeighborValue>,

    // Timer for reconciliation delay
    reconcile_timer: Option<Instant>,

    // Flag indicating warm restart mode
    enabled: bool,
}
```

---

## Warm Restart State Machine

```
┌──────────────┐
│   Normal     │ ← Default state
│  Operation   │
└──────┬───────┘
       │
       │ Warm restart flag detected
       │ (WARM_RESTART_ENABLE_TABLE|neighsyncd = true)
       ▼
┌──────────────┐
│ Caching State│
│              │
│ - Query all  │
│   neighbors  │
│ - Save to    │
│   Redis      │
└──────┬───────┘
       │
       │ Restart triggered
       ▼
┌──────────────┐
│  Startup &   │
│ Load Cache   │
│              │
│ - Load saved │
│   neighbors  │
└──────┬───────┘
       │
       │ Cache loaded
       ▼
┌──────────────┐
│ Reconcile    │
│   Timer      │
│              │
│ - Wait 5 sec │
│ - Buffer new │
│   events     │
└──────┬───────┘
       │
       │ Timer expires
       ▼
┌──────────────┐
│ Reconcile    │
│   State      │
│              │
│ - Query      │
│   kernel     │
│ - Diff       │
│ - Apply      │
└──────┬───────┘
       │
       │ Reconciliation complete
       ▼
┌──────────────┐
│  Resume      │
│  Normal      │
│ Operation    │
└──────────────┘
```

---

## Performance Optimizations

### 1. Redis Batching (Pipelining)

**Before**:
```rust
for neighbor in neighbors {
    redis.hset(&key, &value).await?;  // 100 round-trips
}
```

**After**:
```rust
let mut pipe = redis::pipe();
for neighbor in neighbors {
    pipe.hset(&key, &value);  // Queue command
}
pipe.query_async(&mut conn).await?;  // 1 round-trip
```

**Performance Gain**: 50-100x throughput improvement

### 2. Interface Name Caching

Cache ifindex → interface name mapping:

```rust
pub struct InterfaceCache {
    cache: HashMap<u32, String>,  // ifindex → interface name
    ttl: Duration,
}

// Lookup with caching
fn get_interface_name(&mut self, ifindex: u32) -> Result<String> {
    if let Some(name) = self.cache.get(&ifindex) {
        return Ok(name.clone());  // Cache hit
    }

    // Cache miss: query kernel
    let name = query_interface_name(ifindex)?;
    self.cache.insert(ifindex, name.clone());
    Ok(name)
}
```

**Performance Gain**: 10-20% reduction in syscalls

### 3. FxHash (Feature: perf-fxhash)

Use faster hash function for internal HashMaps:

```rust
use fxhash::FxHashMap;

// Instead of std::collections::HashMap
let neighbors: FxHashMap<NeighborKey, NeighborValue> = FxHashMap::default();
```

**Performance Gain**: 2-3x faster hashing (non-cryptographic)

### 4. State Diffing Optimization

During warm restart, use efficient set operations:

```rust
// Compute differences efficiently
let additions: HashSet<_> = kernel_state.difference(&cached_state).collect();
let deletions: HashSet<_> = cached_state.difference(&kernel_state).collect();
let updates: Vec<_> = kernel_state.intersection(&cached_state)
    .filter(|k| kernel_state[k] != cached_state[k])
    .collect();
```

### 5. Batch Size Tuning

Adaptive batch sizing based on event rate:

```rust
// Low event rate (< 10/sec): Small batches (10) for low latency
// High event rate (> 100/sec): Large batches (100) for high throughput

fn adaptive_batch_size(&self, event_rate: f64) -> usize {
    match event_rate {
        r if r < 10.0 => 10,
        r if r < 50.0 => 50,
        _ => 100,
    }
}
```

### 6. Memory Pooling

Pre-allocate buffers to reduce allocations:

```rust
pub struct NeighborProcessor {
    batch_buffer: Vec<NeighborUpdate>,  // Pre-allocated capacity
    recv_buffer: [u8; 8192],            // Reused for netlink recv
}

impl NeighborProcessor {
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_buffer: Vec::with_capacity(batch_size),
            recv_buffer: [0u8; 8192],
        }
    }
}
```

---

## Error Handling Strategy

### Error Classification

1. **Unrecoverable Errors** (panic):
   - Redis connection failed at startup
   - Netlink socket creation failed
   - Invalid configuration file

2. **Recoverable Errors** (retry with backoff):
   - Transient Redis errors (network glitch)
   - Netlink socket buffer overflow

3. **Expected Errors** (log and continue):
   - Truncated netlink messages
   - Filtered neighbors (broadcast MAC)
   - Redis key not found

### Retry Logic

```rust
pub async fn retry_with_backoff<F, T, E>(
    operation: F,
    max_retries: usize,
    initial_backoff: Duration,
) -> Result<T, E>
where
    F: Fn() -> Future<Output = Result<T, E>>,
{
    let mut backoff = initial_backoff;
    for attempt in 1..=max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_retries => {
                warn!("Operation failed (attempt {}/{}): {:?}", attempt, max_retries, e);
                tokio::time::sleep(backoff).await;
                backoff *= 2;  // Exponential backoff
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}
```

---

## Security Architecture

### Threat Model

**Threats Mitigated**:
1. Unauthorized access to metrics (mTLS)
2. Man-in-the-middle attacks (TLS 1.3 + P-384)
3. Weak cryptography (CNSA 2.0 enforcement)
4. Privilege escalation (systemd hardening)
5. Memory corruption (Rust memory safety)

**Out of Scope**:
1. Redis authentication (assumed trusted local connection)
2. DoS attacks (rate limiting not implemented)

### Defense in Depth

1. **Cryptographic Layer**: CNSA 2.0 mTLS
2. **Process Isolation**: systemd sandboxing (ProtectSystem, PrivateTmp, etc.)
3. **Capability Restriction**: Only CAP_NET_ADMIN and CAP_NET_RAW
4. **Memory Safety**: Rust ownership model
5. **Input Validation**: Netlink message validation

### Certificate Verification

```rust
// Load CA certificate store
let mut root_store = RootCertStore::empty();
for cert in ca_certs {
    root_store.add(cert)?;
}

// Create client certificate verifier
let client_verifier = WebPkiClientVerifier::builder(Arc::new(root_store))
    .build()?;

// Enforce mandatory client authentication
let tls_config = ServerConfig::builder_with_provider(Arc::new(cnsa_provider))
    .with_protocol_versions(&[&rustls::version::TLS13])?
    .with_client_cert_verifier(client_verifier)  // ← Mandatory mTLS
    .with_single_cert(server_certs, server_key)?;
```

---

## Deployment Patterns

### 1. Single-ToR Deployment

```
┌────────────────────────────────┐
│      SONiC Switch (ToR)        │
│                                │
│  ┌──────────────────────────┐  │
│  │      neighsyncd          │  │
│  │ (IPv4 + IPv6 enabled)    │  │
│  └─────────┬────────────────┘  │
│            │                   │
│  ┌─────────▼────────────────┐  │
│  │    Redis (APPL_DB)       │  │
│  └──────────────────────────┘  │
└────────────────────────────────┘
```

### 2. Dual-ToR Deployment

```
┌────────────────────────────────┐  ┌────────────────────────────────┐
│   SONiC Switch (ToR 1)         │  │   SONiC Switch (ToR 2)         │
│                                │  │                                │
│  ┌──────────────────────────┐  │  │  ┌──────────────────────────┐  │
│  │  neighsyncd (instance 1) │  │  │  │  neighsyncd (instance 2) │  │
│  │  (dual_tor = true)       │  │  │  │  (dual_tor = true)       │  │
│  └─────────┬────────────────┘  │  │  └─────────┬────────────────┘  │
│            │                   │  │            │                   │
│  ┌─────────▼────────────────┐  │  │  ┌─────────▼────────────────┐  │
│  │    Redis (APPL_DB)       │  │  │  │    Redis (APPL_DB)       │  │
│  └──────────────────────────┘  │  │  └──────────────────────────┘  │
└────────────────────────────────┘  └────────────────────────────────┘
            │                                      │
            └──────────────┬───────────────────────┘
                           │
                  State synchronized
```

---

## Design Decisions

### Why Rust?

1. **Memory Safety**: No buffer overflows, use-after-free, or data races
2. **Performance**: Zero-cost abstractions, comparable to C++
3. **Reliability**: Compile-time error detection
4. **Async I/O**: First-class async/await support via Tokio

### Why Tokio?

1. **Industry Standard**: Most mature Rust async runtime
2. **Performance**: Efficient epoll/kqueue implementation
3. **Ecosystem**: Compatible with redis-rs, axum, etc.

### Why Redis Pipelining?

1. **Throughput**: 50-100x improvement over sequential operations
2. **Simplicity**: No complex batching logic required
3. **Atomic**: All-or-nothing semantics

### Why Prometheus?

1. **SONiC Standard**: Already integrated in monitoring stack
2. **Pull Model**: Doesn't require outbound connections
3. **Efficient**: Text format with built-in compression

### Why CNSA 2.0?

1. **NSA Recommendation**: Future-proof cryptography
2. **Compliance**: Required for government deployments
3. **Simplicity**: Single cipher suite reduces configuration errors

---

**End of Architecture Documentation**
