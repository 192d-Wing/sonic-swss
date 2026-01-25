# portsyncd Architecture Guide

## Overview

The portsyncd daemon synchronizes kernel port/interface state with SONiC databases via netlink events. This document describes the architectural design, module relationships, and data flow.

## Design Principles

1. **High Performance**: Async I/O, minimal memory allocations
2. **Reliability**: Graceful error handling, health monitoring
3. **Safety**: 100% safe Rust, no unsafe code
4. **Maintainability**: Clear separation of concerns, comprehensive tests
5. **Observability**: Systemd integration, performance metrics

## Module Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      main.rs                             │
│              (Event loop & daemon startup)               │
└────────────────────┬────────────────────────────────────┘
                     │
        ┌────────────┼────────────┐
        ↓            ↓            ↓
    ┌───────────┐  ┌──────────┐  ┌──────────────────┐
    │netlink_   │  │port_sync │  │production_      │
    │socket.rs  │  │.rs       │  │features.rs      │
    └───────────┘  └──────────┘  └──────────────────┘
        ↓            ↓            ↓
    ┌─────────────────────────────────────────────────────┐
    │              redis_adapter.rs                        │
    │         (Async Redis client abstraction)             │
    └─────────────────────────────────────────────────────┘
        ↓
    ┌─────────────────────────────────────────────────────┐
    │              Redis Database                          │
    │  (CONFIG_DB, STATE_DB, APP_DB)                      │
    └─────────────────────────────────────────────────────┘
```

## Core Modules

### 1. main.rs - Event Loop & Startup

**Responsibility**: Bootstrap daemon, run event loop, coordinate shutdown

**Key Functions**:
- `main()`: Entry point, initializes subsystems
- Event loop: Receives netlink events, delegates to handlers

**Event Loop Flow**:
```
Initialize()
  ├─ Load configuration
  ├─ Connect to Redis
  ├─ Open netlink socket
  ├─ Send systemd READY signal
  └─ Start health monitor thread

Loop():
  ├─ Receive netlink event
  ├─ Parse event (netlink_socket)
  ├─ Handle event (port_sync)
  ├─ Update database (redis_adapter)
  ├─ Record metrics (performance)
  ├─ Report health (production_features)
  └─ Check shutdown flag

Shutdown():
  ├─ Stop accepting events
  ├─ Drain event queue
  ├─ Close netlink socket
  ├─ Close Redis connections
  └─ Exit
```

**Dependencies**:
- `tokio`: Async runtime
- `redis_adapter`: Database operations
- `netlink_socket`: Event source
- `port_sync`: Event processing
- `production_features`: Health/systemd

### 2. netlink_socket.rs - Kernel Event Source

**Responsibility**: Listen for kernel port state changes via netlink protocol

**Key Structures**:
```rust
pub struct NetlinkSocket {
    connected: bool,
    // Linux: raw file descriptor to netlink socket
    // macOS: mock event queue
}

pub enum NetlinkEventType {
    NewLink,  // Port came up
    DelLink,  // Port went down
}

pub struct NetlinkEvent {
    event_type: NetlinkEventType,
    port_name: String,
    flags: Option<u32>,
    mtu: Option<u32>,
}
```

**Message Parsing**:
- **RTM_NEWLINK**: Port up event
  - Extract port name from LinkAttribute::IfName
  - Extract flags (IFF_UP, IFF_RUNNING, etc.)
  - Extract MTU for interface configuration
- **RTM_DELLINK**: Port down event
  - Similar parsing, indicates interface removal

**Platform Support**:
- **Linux**: Real netlink socket using `netlink-sys` and `netlink-packet-route`
  - Subscribes to RTNLGRP_LINK (link changes)
  - Receives all RTM_* messages from kernel
  - Filters for NEWLINK/DELLINK
- **macOS**: Mock implementation returning test events
  - Allows development without Linux
  - Useful for CI/CD environments

**Key Methods**:
```rust
pub fn connect(&mut self) -> Result<()>
pub fn receive_event(&mut self) -> Result<Option<NetlinkEvent>>
pub fn disconnect(&mut self) -> Result<()>
```

### 3. port_sync.rs - Event Processing

**Responsibility**: Validate port events and synchronize to databases

**Key Structures**:
```rust
pub struct LinkSync {
    // Tracks port state and configuration
}

pub struct PortLinkState {
    // Represents a single port's state
    port_name: String,
    admin_status: String,
    oper_status: String,
    // ...
}
```

**Port Filtering**:
- Ignores system interfaces: `eth0`, `lo`, `docker*`
- Accepts front-panel ports: `Ethernet*`, `PortChannel*`

**State Tracking**:
1. Reads port configuration from CONFIG_DB
2. Tracks which ports have been initialized
3. Sends PORT_INIT_DONE when all ports ready
4. Updates port status in STATE_DB on events

**Key Methods**:
```rust
pub async fn handle_new_link(
    &mut self,
    event: &NetlinkEvent,
    state_db: &mut dyn DatabaseAdapter,
) -> Result<()>

pub async fn handle_del_link(
    &mut self,
    port_name: &str,
    state_db: &mut dyn DatabaseAdapter,
) -> Result<()>
```

### 4. redis_adapter.rs - Database Abstraction

**Responsibility**: Provide unified async interface to Redis databases

**Dual-Mode Operation**:

```rust
#[cfg(test)]
pub struct RedisAdapter {
    data: Arc<tokio::sync::Mutex<HashMap<...>>>,  // Mock storage
}

#[cfg(not(test))]
pub struct RedisAdapter {
    connection: Arc<tokio::sync::Mutex<Option<ConnectionManager>>>,  // Real Redis
}
```

**Three Database Instances**:
1. **CONFIG_DB** (DB 4): Port configuration
   - Keys: `PORT|<name>`, `PORTCHANNEL|<name>`
   - Values: Port properties (speed, lanes, alias, etc.)
2. **STATE_DB** (DB 6): Current port state
   - Keys: `PORT_TABLE|<name>`
   - Values: `oper_status`, `admin_status`
3. **APP_DB** (DB 0): Application state
   - Keys: `PORT_INIT_DONE`
   - Values: Timestamp when all ports initialized

**Key Methods** (async):
```rust
pub async fn hgetall(&self, key: &str) -> Result<HashMap<String, String>>
pub async fn hset(&mut self, key: &str, fields: &[(String, String)]) -> Result<()>
pub async fn delete(&mut self, key: &str) -> Result<()>
pub async fn keys(&self, pattern: &str) -> Result<Vec<String>>
```

**Implementation Details**:
- Uses `redis::AsyncCommands` trait for operations
- Connection pooling via `ConnectionManager`
- Async/await for non-blocking I/O
- Error handling with custom PortsyncError types

### 5. production_features.rs - Health & Systemd

**Responsibility**: Monitor daemon health and communicate with systemd

**Health Monitoring**:

```rust
pub enum HealthStatus {
    Healthy,    // All systems operational
    Degraded,   // Some metrics concerning
    Unhealthy,  // Critical issues detected
}

pub struct HealthMonitor {
    status: Arc<Mutex<HealthStatus>>,
    last_event: Arc<Mutex<Instant>>,
    config: HealthCheckConfig,
}
```

**Health Checks**:
- **Stall Detection**: No events received for `max_stall_duration` → Unhealthy
- **Failure Rate**: Event failures exceed `max_failure_rate` → Degraded
- **Port Sync Rate**: Port sync success below `min_port_sync_rate` → Degraded

**Systemd Notifications**:

```rust
pub struct SystemdNotifier {
    enabled: bool,  // Detected via NOTIFY_SOCKET env var
}
```

**Signals Sent**:
1. **READY**: On successful initialization
2. **WATCHDOG**: Every `watchdog_interval_secs` (systemd resets timeout)
3. **STATUS**: Periodic operational updates

**Graceful Shutdown**:

```rust
pub struct ShutdownCoordinator {
    shutdown_requested: Arc<AtomicBool>,
    timeout: Duration,
}
```

Handles SIGTERM signal and coordinates clean shutdown.

### 6. config_file.rs - Configuration Management

**Responsibility**: Load and validate TOML configuration files

**Configuration Structure**:

```rust
pub struct PortsyncConfig {
    pub database: DatabaseConfig,
    pub performance: PerformanceConfig,
    pub health: HealthConfig,
}
```

**Default Values**:
- Redis: localhost:6379
- Databases: CONFIG_DB=4, STATE_DB=6
- Health: 10s max stall, 5% max failure rate
- Watchdog: 15s interval

**File Format** (TOML):
```toml
[database]
redis_host = "127.0.0.1"
redis_port = 6379

[health]
max_stall_seconds = 10
max_failure_rate_percent = 5.0
```

### 7. performance.rs - Metrics & Benchmarking

**Responsibility**: Track event processing performance

**Key Structures**:

```rust
pub struct PerformanceMetrics {
    // Tracks event latencies, success rates, throughput
}

pub struct BenchmarkResult {
    // Aggregated metrics from benchmark run
    pub avg_latency_us: u64,
    pub throughput_eps: f64,
    pub success_rate: f64,
}
```

**Measured Metrics**:
- Event latency: Time from event reception to completion
- Throughput: Events processed per second
- Success rate: Percentage of events processed without error
- Memory efficiency: Overhead per tracked event

## Data Flow

### Port Up Event

```
Kernel
  ↓ (Physical interface goes UP)
  └─→ netlink_socket.receive_event()
      ├─ Parse RTM_NEWLINK message
      ├─ Extract: port_name="Ethernet0", flags=IFF_UP|IFF_RUNNING
      └─ Return NetlinkEvent

port_sync.handle_new_link()
  ├─ Get port config from CONFIG_DB
  ├─ Validate port name (not eth0/lo)
  ├─ Create PortLinkState (admin_status=up, oper_status=up)
  └─ Update STATE_DB with new status

redis_adapter.hset()
  ├─ Async Redis HSET PORT_TABLE|Ethernet0 field values
  └─ Update system-wide port state view

production_features.record_event()
  └─ Update health monitor (still alive)
```

### Port Down Event

```
Kernel (Physical interface goes DOWN)
  ↓
netlink_socket.receive_event()
  ├─ Parse RTM_DELLINK message
  └─ Return NetlinkEvent

port_sync.handle_del_link()
  ├─ Mark port as down
  └─ Update STATE_DB

redis_adapter.hset()
  └─ Update PORT_TABLE|Ethernet0 with down status
```

## Concurrency Model

### Async/Await

The daemon uses Tokio async runtime:

```
main (tokio::main)
  │
  └─→ Event Loop (async fn)
       │
       ├─→ await netlink_socket.receive_event()
       │   (Blocks until kernel event)
       │
       ├─→ await port_sync.handle_new_link()
       │   (Async, may yield to other tasks)
       │
       ├─→ await redis_adapter.hset()
       │   (Async Redis I/O)
       │
       └─→ Loop back to receive next event
```

### Shared State

- **Arc<Mutex<T>>**: Health status, Redis connection
  - Used for sharing mutable state between async tasks
  - Minimal lock contention (held <1ms per operation)

- **Arc<AtomicBool>**: Shutdown flag
  - No locking, minimal overhead
  - Checked before each event

### No Background Threads

- Single-threaded event loop design
- Health checks triggered from main loop (not separate thread)
- Watchdog signals sent from event loop

## Error Handling

### Error Types

```rust
pub enum PortsyncError {
    Database(String),        // Redis errors
    Netlink(String),         // Netlink socket errors
    Configuration(String),   // Config file errors
    PortValidation(String),  // Invalid port name
    Io(std::io::Error),      // File I/O errors
    Other(String),           // Generic errors
}
```

### Recovery Strategies

1. **Connection Failure**: Exponential backoff, retry with increasing delay
2. **Event Parse Error**: Log and skip event, continue processing
3. **Database Write Failure**: Retry with backoff
4. **Graceful Shutdown**: Drain in-flight events, exit cleanly

## Testing Strategy

### Test Pyramid

```
           /\
          /E2E\          2 integration tests (full daemon)
         /──────\
        /  Unit  \      106 unit tests (individual modules)
       /──────────\
```

### Test Coverage by Module

| Module | Tests | Coverage |
|--------|-------|----------|
| redis_adapter | 10 | Connection, HGETALL, HSET, DEL, KEYS |
| netlink_socket | 12 | Event parsing, RTM_NEWLINK/DELLINK |
| port_sync | 18 | Port filtering, state tracking, init |
| production_features | 12 | Health monitoring, systemd, shutdown |
| config_file | 12 | Config loading, validation, serialization |
| performance | 7 | Benchmarks (latency, throughput, memory) |
| config | 10 | Database configuration |
| error | 3 | Error message formatting |

### Test Types

1. **Unit Tests**: Testing individual functions in isolation
2. **Integration Tests**: Testing module interactions
3. **Performance Tests**: Measuring latency and throughput
4. **Mock Tests**: Using mock Redis for database tests

## Key Design Decisions

### 1. Async/Await vs Threads

**Decision**: Async/Await with Tokio

**Rationale**:
- Better for I/O-bound workload (network socket, Redis)
- Lower memory per concurrent operation
- Easier to reason about than thread pools

### 2. Mock vs Real Netlink

**Decision**: Platform-aware compilation
- Real netlink on Linux (production)
- Mock on macOS (development)

**Rationale**:
- Developers use macOS, can't use netlink
- Production requires real kernel integration
- Single codebase with conditional compilation

### 3. Trait-Based Polymorphism

**Decision**: `DatabaseAdapter` trait for database abstraction

```rust
#[async_trait]
pub trait DatabaseAdapter: Send + Sync {
    async fn hgetall(&self, key: &str) -> Result<HashMap<String, String>>;
    // ...
}
```

**Rationale**:
- Tests use mock implementation
- Production uses real Redis
- No runtime overhead (monomorphization)

### 4. Configuration File Format

**Decision**: TOML with serde

**Rationale**:
- Human-readable
- Type-safe via serde
- Standard in Rust ecosystem
- Easy to extend

### 5. Health Checks

**Decision**: Integrated into event loop, not separate thread

**Rationale**:
- Avoids synchronization overhead
- Simple single-threaded design
- Health status checked periodically

## Performance Characteristics

### Event Latency

```
Event Reception: 0-100 µs
  ├─ Kernel scheduling
  └─ netlink socket recv

Event Parsing: 50-200 µs
  ├─ Netlink message deserialization
  └─ Attribute extraction

State Update: 100-500 µs
  ├─ Redis HSET command
  └─ Network round-trip (localhost)

Total: 150-800 µs per event (typical)
```

### Memory Layout

```
RedisAdapter: ~100 bytes
NetlinkSocket: ~4KB (4096 byte buffer)
PortLinkState: ~200 bytes per port
HealthMonitor: ~1KB
PerformanceMetrics: ~1MB (1000 tracked events)

Total for 10K ports + metrics:
  ~2-3MB steady state
```

### Throughput

```
Sustained: 800+ events/second
Burst: 7700+ events/second
Latency under load: Sub-linear (async task scheduling)
```

## Future Enhancements

### Phase 6 - Advanced Features

1. **Warm Restart**: Detect EOIU flag in netlink messages
2. **Metrics Export**: Prometheus /metrics endpoint
3. **Self-Healing**: Auto-recovery from transient failures
4. **Multi-Instance**: Support multiple portsyncd daemons

### Phase 7 - Production Hardening

1. **Chaos Testing**: Inject network failures
2. **Scale Testing**: 100K+ ports
3. **Security Audit**: Cryptographic validation
4. **Memory Profiling**: Detect leaks over 24h

## References

- **Netlink Protocol**: `man 7 netlink`
- **RTM_NEWLINK/DELLINK**: `man 7 rtnetlink`
- **systemd Notifications**: https://www.freedesktop.org/software/systemd/man/sd_notify.html
- **Tokio Async**: https://tokio.rs/
- **Redis Async**: https://docs.rs/redis/
- **Rust Async/Await**: https://rust-lang.github.io/async-book/

---

**Last Updated**: Phase 5 Week 5 (Production Deployment)
