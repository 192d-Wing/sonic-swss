# neighsyncd Configuration Guide

**Version:** 1.0
**Last Updated:** 2026-01-25

## Table of Contents

1. [Overview](#overview)
2. [Configuration File Location](#configuration-file-location)
3. [Configuration Format](#configuration-format)
4. [Redis Configuration](#redis-configuration)
5. [Netlink Configuration](#netlink-configuration)
6. [Logging Configuration](#logging-configuration)
7. [Performance Configuration](#performance-configuration)
8. [Metrics and Monitoring](#metrics-and-monitoring)
9. [Deployment Configuration](#deployment-configuration)
10. [Security Configuration](#security-configuration)
11. [Advanced Configuration](#advanced-configuration)
12. [Environment Variables](#environment-variables)
13. [Feature Flags](#feature-flags)
14. [Configuration Examples](#configuration-examples)
15. [Validation](#validation)

---

## Overview

neighsyncd supports configuration through:

1. **Configuration File** (TOML format): `/etc/sonic/neighsyncd/neighsyncd.conf`
2. **Environment Variables**: Override specific settings
3. **Compile-Time Features**: Enable/disable features at build time

Configuration is **optional** - neighsyncd uses sensible defaults for all settings.

---

## Configuration File Location

Default locations (checked in order):

1. `/etc/sonic/neighsyncd/neighsyncd.conf` (production)
2. `./neighsyncd.conf` (current directory, for testing)
3. Environment variable `NEIGHSYNCD_CONFIG`

Example:

```bash
# Use custom configuration file:
export NEIGHSYNCD_CONFIG="/path/to/custom.conf"
/usr/local/bin/sonic-neighsyncd
```

---

## Configuration Format

neighsyncd uses TOML (Tom's Obvious Minimal Language) for configuration.

**TOML Syntax**:

```toml
# Comments start with #

# Key-value pairs
key = "value"
number = 42
boolean = true

# Sections
[section_name]
option1 = "value1"
option2 = 123

# Nested sections
[parent.child]
nested_option = true

# Arrays
list = ["item1", "item2", "item3"]
```

**Example neighsyncd.conf**:

```toml
[redis]
host = "::1"
port = 6379

[logging]
level = "info"
format = "json"

[metrics]
enabled = true
port = 9091
```

---

## Redis Configuration

### Section: `[redis]`

Configure Redis connection parameters.

```toml
[redis]
# Redis server host
# Type: string (IP address or hostname)
# Default: "::1" (IPv6 loopback)
# Examples: "::1", "127.0.0.1", "redis.local"
host = "::1"

# Redis server port
# Type: integer
# Default: 6379
# Range: 1-65535
port = 6379

# Redis database number
# Type: integer
# Default: 0 (APPL_DB in SONiC)
# Range: 0-15
database = 0

# Connection timeout in milliseconds
# Type: integer
# Default: 5000 (5 seconds)
# Range: 100-60000
timeout_ms = 5000

# Maximum number of reconnection attempts
# Type: integer
# Default: 10
# Range: 1-100
max_retries = 10

# Retry backoff in milliseconds
# Type: integer
# Default: 1000 (1 second)
# Range: 100-10000
retry_backoff_ms = 1000
```

### Redis Connection String

Alternative to individual settings:

```toml
[redis]
# Full connection string
# Format: redis://[username:password@]host[:port][/database]
# Example: redis://::1:6379/0
url = "redis://::1:6379/0"
```

### Redis Tuning

```toml
[redis]
# Enable TCP keepalive
# Default: true
tcp_keepalive = true

# TCP keepalive interval (seconds)
# Default: 60
tcp_keepalive_interval = 60

# Connection pool size
# Default: 4
# Range: 1-32
pool_size = 4

# Pipeline batch size
# Default: 100
# Range: 10-1000
pipeline_batch_size = 100
```

---

## Netlink Configuration

### Section: `[netlink]`

Configure netlink socket parameters.

```toml
[netlink]
# Socket receive buffer size in bytes
# Larger buffer reduces packet loss under high event rates
# Type: integer
# Default: 262144 (256 KB)
# Recommended:
#   - < 100 interfaces: 262144 (256 KB)
#   - 100-500 interfaces: 524288 (512 KB)
#   - 500+ interfaces: 1048576 (1 MB)
socket_buffer_size = 262144

# Netlink socket timeout in milliseconds
# Type: integer
# Default: 5000 (5 seconds)
# Range: 1000-60000
timeout_ms = 5000

# Netlink multicast groups to subscribe
# Type: array of strings
# Default: ["neigh"]
# Options: "neigh", "link", "route"
groups = ["neigh"]

# Enable strict netlink checking
# Type: boolean
# Default: true
strict_checking = true
```

### Socket Buffer Size Calculation

Recommended buffer size based on expected event rate:

```
Event Rate (events/sec) × Average Message Size (bytes) × Buffer Time (seconds) = Buffer Size

Examples:
  100 events/sec × 192 bytes × 5 sec = 96,000 bytes (~100 KB)
  1000 events/sec × 192 bytes × 5 sec = 960,000 bytes (~1 MB)
```

Kernel maximum buffer size:

```bash
# Check maximum allowed buffer size:
sysctl net.core.rmem_max
# Increase if needed:
sudo sysctl -w net.core.rmem_max=2097152  # 2 MB
```

---

## Logging Configuration

### Section: `[logging]`

Configure structured logging.

```toml
[logging]
# Log level
# Type: string (case-insensitive)
# Default: "info"
# Options:
#   - "trace": Extremely verbose (function entry/exit)
#   - "debug": Detailed debugging information
#   - "info": Informational messages (production default)
#   - "warn": Warning messages only
#   - "error": Error messages only
level = "info"

# Log format
# Type: string
# Default: "json"
# Options:
#   - "json": Structured JSON (for log aggregation)
#   - "text": Human-readable plain text
#   - "compact": Compact single-line format
format = "json"

# Log target
# Type: string
# Default: "journald" (systemd journal)
# Options:
#   - "stdout": Standard output
#   - "stderr": Standard error
#   - "file": Log file (requires file_path)
#   - "journald": systemd journal
target = "journald"

# Log file path (only used if target = "file")
# Type: string
# Default: "/var/log/sonic/neighsyncd/neighsyncd.log"
file_path = "/var/log/sonic/neighsyncd/neighsyncd.log"

# Enable log rotation (only for target = "file")
# Type: boolean
# Default: true
rotate = true

# Log file rotation size in bytes
# Type: integer
# Default: 10485760 (10 MB)
max_file_size = 10485760

# Number of rotated log files to keep
# Type: integer
# Default: 5
max_backups = 5
```

### Log Level Guidance

| Level | When to Use | Performance Impact |
|-------|-------------|-------------------|
| `trace` | Deep debugging, troubleshooting race conditions | Very High (50-100% overhead) |
| `debug` | Troubleshooting issues, development | High (20-30% overhead) |
| `info` | Production default, normal operations | Low (< 5% overhead) |
| `warn` | Production (quiet), only issues | Very Low (< 1% overhead) |
| `error` | Critical errors only | Minimal |

### Structured Logging Example

JSON format (production):

```json
{"timestamp":"2026-01-25T10:30:45.123Z","level":"INFO","target":"neighsyncd","fields":{"message":"Neighbor added","interface":"Ethernet0","ip":"2001:db8::1","mac":"aa:bb:cc:dd:ee:ff"}}
```

Text format (development):

```
2026-01-25 10:30:45 INFO neighsyncd: Neighbor added interface=Ethernet0 ip=2001:db8::1 mac=aa:bb:cc:dd:ee:ff
```

---

## Performance Configuration

### Section: `[performance]`

Tune performance parameters.

```toml
[performance]
# Batch size for Redis operations
# Larger batches improve throughput but increase latency
# Type: integer
# Default: 100
# Range: 10-1000
# Recommended:
#   - Low event rate (< 10/sec): 10-50
#   - Medium event rate (10-100/sec): 50-100
#   - High event rate (> 100/sec): 100-500
batch_size = 100

# Batch timeout in milliseconds
# Flush batch even if not full after this timeout
# Type: integer
# Default: 100
# Range: 10-1000
batch_timeout_ms = 100

# Warm restart reconciliation timeout in milliseconds
# Time to wait before reconciling cached state with kernel state
# Type: integer
# Default: 5000 (5 seconds)
# Range: 1000-30000
reconcile_timeout_ms = 5000

# Event queue depth
# Maximum number of pending neighbor events
# Type: integer
# Default: 10000
# Range: 1000-100000
queue_depth = 10000

# Worker threads for async runtime
# Type: integer
# Default: 4
# Range: 1-32
# Special value: 0 = use all available CPU cores
worker_threads = 4

# Enable work stealing scheduler
# Type: boolean
# Default: true
work_stealing = true
```

### Performance Tuning Matrix

| Event Rate | Batch Size | Batch Timeout | Worker Threads | Latency | Throughput |
|------------|------------|---------------|----------------|---------|------------|
| Low (< 10/sec) | 10 | 50 ms | 2 | **Low** | Medium |
| Medium (10-100/sec) | 50 | 100 ms | 4 | Medium | **High** |
| High (> 100/sec) | 100 | 100 ms | 8 | High | **Very High** |
| Burst (spikes) | 100 | 50 ms | 8 | Medium | **Very High** |

---

## Metrics and Monitoring

### Section: `[metrics]`

Configure Prometheus metrics endpoint.

```toml
[metrics]
# Enable Prometheus metrics endpoint
# Type: boolean
# Default: true
enabled = true

# Metrics server port (IPv6 loopback [::1])
# Type: integer
# Default: 9091
# Range: 1024-65535
port = 9091

# Bind address
# Type: string
# Default: "::1" (IPv6 loopback, local access only)
# Options: "::1", "::0" (all IPv6), "127.0.0.1", "0.0.0.0" (all IPv4)
bind_address = "::1"

# Enable mTLS for metrics endpoint (CNSA 2.0 compliant)
# Type: boolean
# Default: true (highly recommended for production)
mtls_enabled = true

# Server certificate path (PEM format)
# Type: string
# Required if mtls_enabled = true
# Must be CNSA 2.0 compliant (EC P-384/P-521, SHA-384+)
server_cert = "/etc/sonic/metrics/server/server-cert.pem"

# Server private key path (PEM format)
# Type: string
# Required if mtls_enabled = true
server_key = "/etc/sonic/metrics/server/server-key.pem"

# CA certificate for client verification (PEM format)
# Type: string
# Required if mtls_enabled = true
ca_cert = "/etc/sonic/metrics/ca/ca-cert.pem"

# Enable health check endpoint
# Type: boolean
# Default: true
health_enabled = true

# Health check path
# Type: string
# Default: "/health"
health_path = "/health"

# Metrics export format
# Type: string
# Default: "prometheus"
# Options: "prometheus", "json"
export_format = "prometheus"
```

### Health Monitor Configuration

```toml
[metrics.health]
# Maximum stall duration before marking as unhealthy
# Type: integer (milliseconds)
# Default: 10000 (10 seconds)
max_stall_duration_ms = 10000

# Maximum failure rate before marking as degraded
# Type: float (0.0-1.0)
# Default: 0.05 (5%)
max_failure_rate = 0.05

# Minimum events for failure rate calculation
# Type: integer
# Default: 100
min_events_for_rate = 100
```

---

## Deployment Configuration

### Section: `[deployment]`

Configure deployment-specific settings.

```toml
[deployment]
# Enable dual-ToR support
# Type: boolean
# Default: false
# Set to true for multi-instance neighbor synchronization
dual_tor = false

# Chassis name (for dual-ToR deployments)
# Type: string
# Default: "sonic"
chassis_name = "sonic"

# Enable IPv4 neighbor synchronization
# Type: boolean
# Default: true
ipv4_enabled = true

# Enable IPv6 neighbor synchronization
# Type: boolean
# Default: true
ipv6_enabled = true

# Deployment mode
# Type: string
# Default: "production"
# Options: "production", "development", "testing"
mode = "production"

# Hostname (override automatic detection)
# Type: string (optional)
# Default: auto-detected from system
# hostname = "sonic-switch-01"
```

### Dual-ToR Configuration

For dual-ToR deployments, zero MAC addresses are allowed:

```toml
[deployment]
dual_tor = true
chassis_name = "sonic-tor1"

[filtering]
# Allow zero MAC in dual-ToR mode
allow_zero_mac = true
```

---

## Security Configuration

### Section: `[security]`

Configure security and compliance settings.

```toml
[security]
# Enforce CNSA 2.0 compliance for all TLS connections
# Type: boolean
# Default: true
cnsa_enforcement = true

# Allowed TLS versions
# Type: array of strings
# Default: ["TLSv1.3"]
# CNSA 2.0 requires: TLS 1.3 only
tls_versions = ["TLSv1.3"]

# Allowed cipher suites
# Type: array of strings
# Default: ["TLS_AES_256_GCM_SHA384"]
# CNSA 2.0 requires: TLS_AES_256_GCM_SHA384 only
cipher_suites = ["TLS_AES_256_GCM_SHA384"]

# Minimum elliptic curve size (bits)
# Type: integer
# Default: 384
# CNSA 2.0 requires: P-384 (384 bits) or P-521 (521 bits)
# Range: 256, 384, 521
min_ec_key_size = 384

# Disable TLS session resumption (maximum security)
# Type: boolean
# Default: true
disable_session_resumption = true

# Certificate verification mode
# Type: string
# Default: "strict"
# Options: "strict", "relaxed" (not recommended)
cert_verification = "strict"

# Minimum certificate validity (days)
# Type: integer
# Default: 7 (warn if cert expires within 7 days)
min_cert_validity_days = 7
```

### CNSA 2.0 Compliance Requirements

To maintain CNSA 2.0 compliance:

1. **TLS Version**: TLS 1.3 only (no TLS 1.2 or earlier)
2. **Cipher Suite**: TLS_AES_256_GCM_SHA384 only
3. **Key Exchange**: ECDHE with P-384 or P-521
4. **Certificates**: EC P-384+ with SHA-384+ signatures
5. **Client Authentication**: Mandatory (mTLS)

Non-compliant configurations will be rejected at startup.

---

## Advanced Configuration

### Section: `[advanced]`

Advanced settings (use with caution).

```toml
[advanced]
# Enable memory profiling
# Type: boolean
# Default: false
# Warning: Adds significant overhead, use only for troubleshooting
memory_profiling = false

# Memory profiling interval (seconds)
# Type: integer
# Default: 60
memory_profile_interval = 60

# Enable CPU profiling
# Type: boolean
# Default: false
# Warning: Adds overhead, use only for troubleshooting
cpu_profiling = false

# CPU profiling interval (seconds)
# Type: integer
# Default: 60
cpu_profile_interval = 60

# Enable distributed tracing
# Type: boolean
# Default: false
tracing_enabled = false

# Tracing endpoint (Jaeger, Zipkin, etc.)
# Type: string
# Format: URL
# Example: "http://localhost:9411/api/v2/spans" (Zipkin)
tracing_endpoint = "http://localhost:14268/api/traces"

# Tracing sample rate (0.0-1.0)
# Type: float
# Default: 0.1 (10%)
# Range: 0.0-1.0
tracing_sample_rate = 0.1

# Enable core dumps on panic
# Type: boolean
# Default: false
enable_core_dumps = false

# Core dump path
# Type: string
# Default: "/var/crash/neighsyncd"
core_dump_path = "/var/crash/neighsyncd"
```

---

## Environment Variables

Environment variables override configuration file settings.

### Redis Environment Variables

```bash
# Redis host
export NEIGHSYNCD_REDIS_HOST="::1"

# Redis port
export NEIGHSYNCD_REDIS_PORT="6379"

# Redis database
export NEIGHSYNCD_REDIS_DB="0"

# Redis connection timeout (milliseconds)
export NEIGHSYNCD_REDIS_TIMEOUT="5000"
```

### Logging Environment Variables

```bash
# Log level
export NEIGHSYNCD_LOG_LEVEL="info"

# Log format
export NEIGHSYNCD_LOG_FORMAT="json"

# Rust log override (more granular)
export RUST_LOG="neighsyncd=debug,tokio=info"

# Rust backtrace (for debugging)
export RUST_BACKTRACE="1"  # Enable backtraces
export RUST_BACKTRACE="full"  # Full backtraces
```

### Metrics Environment Variables

```bash
# Metrics port
export NEIGHSYNCD_METRICS_PORT="9091"

# Enable/disable metrics
export NEIGHSYNCD_METRICS_ENABLED="true"

# mTLS enabled
export NEIGHSYNCD_MTLS_ENABLED="true"
```

### Performance Environment Variables

```bash
# Batch size
export NEIGHSYNCD_BATCH_SIZE="100"

# Worker threads
export NEIGHSYNCD_WORKER_THREADS="4"
```

### Priority Order

Configuration priority (highest to lowest):

1. **Environment variables** (highest priority)
2. **Configuration file** (`neighsyncd.conf`)
3. **Compiled defaults** (lowest priority)

Example:

```bash
# Configuration file sets: batch_size = 50
# Environment variable overrides:
export NEIGHSYNCD_BATCH_SIZE="100"

# Actual value used: 100 (environment variable wins)
```

---

## Feature Flags

Compile-time feature flags (set in `Cargo.toml`).

### Available Features

```toml
[features]
default = ["ipv4", "ipv6", "dual-tor"]

# IPv4 support
ipv4 = []

# IPv6 support
ipv6 = []

# Dual-ToR support
dual-tor = []

# Performance: Use FxHash for HashMaps
perf-fxhash = ["fxhash"]

# Performance: Interface batching optimization
perf-interface-batching = []

# Performance: State diffing optimization
perf-state-diffing = []

# Development: Enable extra debug logging
debug-logging = []

# Development: Enable metrics in debug builds
debug-metrics = []
```

### Building with Features

```bash
# Default features (IPv4 + IPv6 + dual-ToR):
cargo build --release -p sonic-neighsyncd

# Only IPv6, no dual-ToR:
cargo build --release -p sonic-neighsyncd \
  --no-default-features --features ipv6

# IPv4 + IPv6 + performance optimizations:
cargo build --release -p sonic-neighsyncd \
  --features perf-fxhash,perf-interface-batching

# All performance features:
cargo build --release -p sonic-neighsyncd \
  --features perf-fxhash,perf-interface-batching,perf-state-diffing
```

### Feature Flag Trade-offs

| Feature | Binary Size | Memory Usage | Performance | Compatibility |
|---------|-------------|--------------|-------------|---------------|
| `ipv4` | +50 KB | +10 KB | - | SONiC |
| `ipv6` | +80 KB | +15 KB | - | SONiC |
| `dual-tor` | +30 KB | +5 KB | - | SONiC |
| `perf-fxhash` | +20 KB | - | +10-20% | All |
| `perf-interface-batching` | +15 KB | +5 KB | +5-10% | All |
| `perf-state-diffing` | +25 KB | +10 KB | +15-25% (warm restart) | All |

---

## Configuration Examples

### Example 1: Production (Single-ToR, mTLS)

```toml
# /etc/sonic/neighsyncd/neighsyncd.conf

[redis]
host = "::1"
port = 6379
database = 0

[netlink]
socket_buffer_size = 262144

[logging]
level = "info"
format = "json"
target = "journald"

[performance]
batch_size = 100
batch_timeout_ms = 100

[metrics]
enabled = true
port = 9091
mtls_enabled = true
server_cert = "/etc/sonic/metrics/server/server-cert.pem"
server_key = "/etc/sonic/metrics/server/server-key.pem"
ca_cert = "/etc/sonic/metrics/ca/ca-cert.pem"

[deployment]
dual_tor = false
ipv4_enabled = true
ipv6_enabled = true

[security]
cnsa_enforcement = true
```

### Example 2: Development (No mTLS, Debug Logging)

```toml
# ./neighsyncd.conf (development)

[redis]
host = "127.0.0.1"
port = 6379

[logging]
level = "debug"
format = "text"
target = "stdout"

[performance]
batch_size = 10  # Small batches for faster iteration

[metrics]
enabled = true
port = 9091
mtls_enabled = false  # Disable mTLS for local testing

[deployment]
mode = "development"
```

### Example 3: High-Performance (Large Deployment)

```toml
# /etc/sonic/neighsyncd/neighsyncd.conf

[redis]
host = "::1"
port = 6379
pool_size = 8

[netlink]
socket_buffer_size = 1048576  # 1 MB for high event rate

[logging]
level = "warn"  # Minimal logging

[performance]
batch_size = 500  # Large batches
batch_timeout_ms = 50
worker_threads = 8  # More workers
queue_depth = 50000  # Large queue

[metrics]
enabled = true
```

### Example 4: Dual-ToR Deployment

```toml
# /etc/sonic/neighsyncd/neighsyncd.conf

[redis]
host = "::1"
port = 6379

[deployment]
dual_tor = true
chassis_name = "sonic-tor1"  # Unique per ToR
ipv4_enabled = true
ipv6_enabled = true

[filtering]
allow_zero_mac = true  # Required for dual-ToR
```

---

## Validation

### Configuration File Validation

Validate configuration before starting:

```bash
# Check configuration syntax:
sonic-neighsyncd --check-config

# Dry-run (validate and exit):
sonic-neighsyncd --dry-run

# Validate and show effective configuration:
sonic-neighsyncd --show-config
```

### Common Validation Errors

```
Error: Invalid batch_size: 5000 (must be between 10 and 1000)
Error: Invalid log level: "trace2" (must be one of: trace, debug, info, warn, error)
Error: mTLS enabled but server_cert not specified
Error: Invalid TOML syntax at line 42: expected '=', found ':'
Error: Certificate file not found: /etc/sonic/metrics/server/server-cert.pem
```

### Configuration Testing

Test configuration changes safely:

```bash
# 1. Backup current configuration:
sudo cp /etc/sonic/neighsyncd/neighsyncd.conf \
       /etc/sonic/neighsyncd/neighsyncd.conf.backup

# 2. Edit configuration:
sudo vi /etc/sonic/neighsyncd/neighsyncd.conf

# 3. Validate:
sonic-neighsyncd --check-config

# 4. Restart with new configuration:
sudo systemctl restart neighsyncd.service

# 5. Monitor logs:
sudo journalctl -u neighsyncd.service -f

# 6. Rollback if needed:
sudo cp /etc/sonic/neighsyncd/neighsyncd.conf.backup \
       /etc/sonic/neighsyncd/neighsyncd.conf
sudo systemctl restart neighsyncd.service
```

---

## Best Practices

1. **Start with defaults**: Only override settings when necessary
2. **Use environment variables for secrets**: Avoid hardcoding credentials
3. **Enable mTLS in production**: Always use CNSA 2.0 compliant mTLS
4. **Monitor metrics**: Use Prometheus to track performance
5. **Test configuration changes**: Use `--dry-run` before deployment
6. **Keep backups**: Always backup configuration before changes
7. **Document customizations**: Comment why each setting was changed
8. **Tune batch size**: Match batch size to event rate
9. **Use structured logging**: JSON format for production
10. **Rotate logs**: Enable log rotation to prevent disk fill

---

**End of Configuration Guide**
