# neighsyncd Monitoring & Observability

This document describes the monitoring and observability features of neighsyncd, including Prometheus metrics, health checks, and CNSA 2.0 compliant mTLS configuration.

## Table of Contents

- [Overview](#overview)
- [Metrics Server](#metrics-server)
- [CNSA 2.0 mTLS Configuration](#cnsa-20-mtls-configuration)
- [Metrics Catalog](#metrics-catalog)
- [Health Monitoring](#health-monitoring)
- [Prometheus Integration](#prometheus-integration)
- [Troubleshooting](#troubleshooting)

## Overview

neighsyncd exposes Prometheus metrics and health status through a secure HTTPS endpoint with mandatory CNSA 2.0 compliant mutual TLS (mTLS). This enables:

- Real-time performance monitoring
- System health tracking
- Alert generation based on metrics
- Integration with existing SONiC monitoring infrastructure

**Security**: The metrics endpoint requires CNSA 2.0 compliant mTLS with client certificate authentication. See [SECURITY.md](SECURITY.md) for compliance details.

## Metrics Server

### Endpoints

The metrics server exposes two endpoints:

#### `/metrics` - Prometheus Metrics
Returns metrics in Prometheus text format for scraping.

```bash
curl --cert client-cert.pem --key client-key.pem --cacert ca-cert.pem \
  https://[::1]:9091/metrics
```

#### `/health` - Health Status
Returns JSON health status with health score.

```bash
curl --cert client-cert.pem --key client-key.pem --cacert ca-cert.pem \
  https://[::1]:9091/health
```

Response format:
```json
{
  "status": "healthy",
  "health_score": 1.0
}
```

Health status values:
- `"healthy"` - health_score >= 1.0 (fully operational)
- `"degraded"` - health_score >= 0.5 (operational with issues)
- `"unhealthy"` - health_score < 0.5 (not operational)

### Configuration

The metrics server is configured via `MetricsServerConfig`:

```rust
pub struct MetricsServerConfig {
    /// Server certificate path (PEM format)
    pub server_cert_path: String,  // Default: /etc/sonic/metrics/server-cert.pem

    /// Server private key path (PEM format, EC P-384 or higher required)
    pub server_key_path: String,   // Default: /etc/sonic/metrics/server-key.pem

    /// CA certificate path for client verification (PEM format)
    pub ca_cert_path: String,      // Default: /etc/sonic/metrics/ca-cert.pem

    /// Port to bind to
    pub port: u16,                 // Default: 9091
}
```

### Starting the Metrics Server

**Production (CNSA 2.0 mTLS - REQUIRED)**:
```rust
use sonic_neighsyncd::{MetricsCollector, MetricsServerConfig, start_metrics_server};

let metrics = MetricsCollector::new()?;
let config = MetricsServerConfig::default();
start_metrics_server(metrics, config).await?;
```

**Development Only (HTTP, NO TLS)**:
```rust
use sonic_neighsyncd::{MetricsCollector, start_metrics_server_insecure};

let metrics = MetricsCollector::new()?;
start_metrics_server_insecure(metrics, Some(9091)).await?;
```

**WARNING**: `start_metrics_server_insecure()` should NEVER be used in production. It provides no encryption or authentication and violates NIST SC-8 (Transmission Confidentiality).

## CNSA 2.0 mTLS Configuration

### Overview

The metrics server implements **mandatory CNSA 2.0 compliant mutual TLS**:

- **TLS 1.3 ONLY** (no TLS 1.2 or earlier)
- **Single cipher suite**: `TLS_AES_256_GCM_SHA384`
- **Mandatory client certificates** (mTLS enforced)
- **AWS-LC-RS crypto provider** (FIPS 140-3 validated)
- **No session resumption** (maximum security)
- **ALPN**: HTTP/2 preferred, HTTP/1.1 fallback

### Certificate Requirements

#### Server Certificate
- **Algorithm**: Elliptic Curve (EC)
- **Curve**: P-384 or P-521 (CNSA 2.0 compliant)
- **Hash**: SHA-384 or SHA-512
- **Key usage**: Digital signature, key encipherment
- **Format**: PEM

#### Server Private Key
- **Algorithm**: EC P-384 or P-521
- **Format**: PEM (PKCS#8 or SEC1)

#### CA Certificate (for Client Verification)
- **Algorithm**: EC P-384 or P-521
- **Hash**: SHA-384 or SHA-512
- **Purpose**: Sign client certificates
- **Format**: PEM

#### Client Certificate (Required for Access)
- **Algorithm**: EC P-384 or P-521
- **Hash**: SHA-384 or SHA-512
- **Signed by**: CA certificate configured on server
- **Key usage**: Digital signature, client authentication
- **Format**: PEM

### Certificate Generation

#### Generate CA Certificate (once per deployment)
```bash
# Generate CA private key (P-384)
openssl ecparam -name secp384r1 -genkey -noout -out ca-key.pem

# Generate CA certificate (SHA-384, 10 year validity)
openssl req -new -x509 -sha384 -key ca-key.pem -out ca-cert.pem -days 3650 \
  -subj "/C=US/O=SONiC/CN=SONiC Metrics CA"
```

#### Generate Server Certificate
```bash
# Generate server private key (P-384)
openssl ecparam -name secp384r1 -genkey -noout -out server-key.pem

# Generate certificate signing request
openssl req -new -sha384 -key server-key.pem -out server.csr \
  -subj "/C=US/O=SONiC/CN=neighsyncd-metrics"

# Sign with CA (SHA-384, 1 year validity)
openssl x509 -req -in server.csr -CA ca-cert.pem -CAkey ca-key.pem \
  -CAcreateserial -out server-cert.pem -days 365 -sha384
```

#### Generate Client Certificate (for Prometheus scraper)
```bash
# Generate client private key (P-384)
openssl ecparam -name secp384r1 -genkey -noout -out client-key.pem

# Generate certificate signing request
openssl req -new -sha384 -key client-key.pem -out client.csr \
  -subj "/C=US/O=SONiC/CN=prometheus-scraper"

# Sign with CA (SHA-384, 1 year validity)
openssl x509 -req -in client.csr -CA ca-cert.pem -CAkey ca-key.pem \
  -CAcreateserial -out client-cert.pem -days 365 -sha384
```

### Certificate Installation

Place certificates in the default locations:
```bash
sudo mkdir -p /etc/sonic/metrics
sudo cp server-cert.pem /etc/sonic/metrics/
sudo cp server-key.pem /etc/sonic/metrics/
sudo cp ca-cert.pem /etc/sonic/metrics/
sudo chmod 600 /etc/sonic/metrics/server-key.pem
sudo chown root:root /etc/sonic/metrics/*
```

### Verifying mTLS Configuration

Test the metrics endpoint with client certificate:
```bash
curl --cert client-cert.pem --key client-key.pem --cacert ca-cert.pem \
  https://[::1]:9091/metrics
```

Verify connection details:
```bash
openssl s_client -connect [::1]:9091 -cert client-cert.pem -key client-key.pem \
  -CAfile ca-cert.pem -showcerts
```

Expected output should show:
```
Protocol  : TLSv1.3
Cipher    : TLS_AES_256_GCM_SHA384
```

## Metrics Catalog

### Counters

Counters increment monotonically and never decrease. Use `rate()` or `irate()` in PromQL to calculate rates.

| Metric | Description | Labels | Example Query |
|--------|-------------|--------|---------------|
| `neighsyncd_neighbors_processed_total` | Total neighbor events processed | none | `rate(neighsyncd_neighbors_processed_total[5m])` |
| `neighsyncd_neighbors_added_total` | Total neighbors added | none | `rate(neighsyncd_neighbors_added_total[5m])` |
| `neighsyncd_neighbors_deleted_total` | Total neighbors deleted | none | `rate(neighsyncd_neighbors_deleted_total[5m])` |
| `neighsyncd_events_failed_total` | Total failed events | none | `rate(neighsyncd_events_failed_total[5m])` |
| `neighsyncd_netlink_errors_total` | Total netlink socket errors | none | `rate(neighsyncd_netlink_errors_total[5m])` |
| `neighsyncd_redis_errors_total` | Total Redis operation errors | none | `rate(neighsyncd_redis_errors_total[5m])` |

### Gauges

Gauges represent current state and can increase or decrease.

| Metric | Description | Values | Alerting Threshold |
|--------|-------------|--------|-------------------|
| `neighsyncd_pending_neighbors` | Current pending neighbor events | integer | > 1000 (backlog) |
| `neighsyncd_queue_depth` | Current event queue depth | integer | > 500 (saturation) |
| `neighsyncd_memory_bytes` | Process memory usage in bytes | integer | > 200MB (leak) |
| `neighsyncd_redis_connected` | Redis connection status | 1=connected, 0=disconnected | == 0 (critical) |
| `neighsyncd_netlink_connected` | Netlink socket status | 1=connected, 0=disconnected | == 0 (critical) |
| `neighsyncd_health_status` | Service health status | 1.0=healthy, 0.5=degraded, 0.0=unhealthy | < 1.0 (warning) |

### Histograms

Histograms track distributions of values with configurable buckets. Use `histogram_quantile()` for percentiles.

| Metric | Description | Buckets | Example Query (p99) |
|--------|-------------|---------|---------------------|
| `neighsyncd_event_latency_seconds` | Event processing latency | 0.1ms - 1s | `histogram_quantile(0.99, rate(neighsyncd_event_latency_seconds_bucket[5m]))` |
| `neighsyncd_redis_latency_seconds` | Redis operation latency | 0.1ms - 1s | `histogram_quantile(0.99, rate(neighsyncd_redis_latency_seconds_bucket[5m]))` |
| `neighsyncd_batch_size` | Distribution of batch sizes | 1 - 1000 | `histogram_quantile(0.95, rate(neighsyncd_batch_size_bucket[5m]))` |

**Histogram buckets**:
- Latency metrics: `[0.0001, 0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]` seconds
- Batch size: `[1, 5, 10, 25, 50, 100, 250, 500, 1000]` events

## Health Monitoring

### Health Status State Machine

neighsyncd tracks health status based on event processing activity and failure rates:

```
Healthy (1.0) → Degraded (0.5) → Unhealthy (0.0)
      ↑               ↑               ↑
      └───────────────┴───────────────┘
           (automatic recovery)
```

### Health Thresholds

#### Stall Detection
- **Trigger**: No events processed for > 10 seconds
- **Status**: Unhealthy (0.0)
- **Cause**: Netlink socket disconnection, kernel neighbor table empty
- **Recovery**: Automatic when events resume

#### Failure Rate Tracking
- **Trigger**: Event failure rate > 5%
- **Status**: Degraded (0.5)
- **Cause**: Redis errors, invalid neighbor entries, processing errors
- **Recovery**: Automatic when failure rate drops below threshold

### Configuration

Health monitoring is configured via `HealthMonitor::with_config()`:

```rust
use sonic_neighsyncd::{HealthMonitor, MetricsCollector};
use std::time::Duration;

let metrics = MetricsCollector::new()?;
let health = HealthMonitor::with_config(
    metrics,
    Duration::from_secs(10),  // max_stall_duration
    0.05,                      // max_failure_rate (5%)
);
```

### Recording Events

```rust
// Record successful event
health.record_success();

// Record failed event
health.record_failure();

// Update health status (checks thresholds)
health.update_health();
```

### Checking Health

```rust
use sonic_neighsyncd::HealthStatus;

match health.status() {
    HealthStatus::Healthy => println!("Service is fully operational"),
    HealthStatus::Degraded => println!("Service is degraded"),
    HealthStatus::Unhealthy => println!("Service is not operational"),
}

// Get detailed metrics
let failure_rate = health.failure_rate();
let time_since_event = health.time_since_last_event();
```

## Prometheus Integration

### Scrape Configuration

Add to Prometheus `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'sonic-neighsyncd'
    scheme: https
    static_configs:
      - targets: ['[::1]:9091']

    # CNSA 2.0 mTLS configuration
    tls_config:
      # Client certificate for authentication
      cert_file: /etc/prometheus/certs/client-cert.pem
      key_file: /etc/prometheus/certs/client-key.pem

      # CA certificate to verify server
      ca_file: /etc/prometheus/certs/ca-cert.pem

      # Enforce TLS 1.3
      min_version: TLS13
      max_version: TLS13

      # Server name for certificate validation
      server_name: neighsyncd-metrics

    scrape_interval: 15s
    scrape_timeout: 10s

    # Relabel for multi-instance deployments
    relabel_configs:
      - source_labels: [__address__]
        target_label: instance
        replacement: 'neighsyncd'
```

### Example Prometheus Queries

#### Event Processing Rate
```promql
# Events per second (5 minute rate)
rate(neighsyncd_neighbors_processed_total[5m])

# Add/delete breakdown
rate(neighsyncd_neighbors_added_total[5m])
rate(neighsyncd_neighbors_deleted_total[5m])
```

#### Error Rates
```promql
# Overall event failure rate
rate(neighsyncd_events_failed_total[5m]) / rate(neighsyncd_neighbors_processed_total[5m])

# Redis error rate
rate(neighsyncd_redis_errors_total[5m])

# Netlink error rate
rate(neighsyncd_netlink_errors_total[5m])
```

#### Latency Analysis
```promql
# p50 event processing latency
histogram_quantile(0.50, rate(neighsyncd_event_latency_seconds_bucket[5m]))

# p95 event processing latency
histogram_quantile(0.95, rate(neighsyncd_event_latency_seconds_bucket[5m]))

# p99 event processing latency
histogram_quantile(0.99, rate(neighsyncd_event_latency_seconds_bucket[5m]))

# p99 Redis latency
histogram_quantile(0.99, rate(neighsyncd_redis_latency_seconds_bucket[5m]))
```

#### System Health
```promql
# Health status (1.0 = healthy)
neighsyncd_health_status

# Connection status
neighsyncd_redis_connected
neighsyncd_netlink_connected

# Resource usage
neighsyncd_memory_bytes / 1024 / 1024  # Memory in MB
neighsyncd_queue_depth
neighsyncd_pending_neighbors
```

### Alerting Rules

Create `neighsyncd_alerts.yml`:

```yaml
groups:
  - name: neighsyncd
    interval: 30s
    rules:
      # Critical: Service unavailable
      - alert: NeighsyncdUnhealthy
        expr: neighsyncd_health_status < 0.5
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "neighsyncd is unhealthy"
          description: "Health status: {{ $value }}"

      # Critical: Redis disconnected
      - alert: NeighsyncdRedisDown
        expr: neighsyncd_redis_connected == 0
        for: 30s
        labels:
          severity: critical
        annotations:
          summary: "neighsyncd Redis connection lost"

      # Critical: Netlink disconnected
      - alert: NeighsyncdNetlinkDown
        expr: neighsyncd_netlink_connected == 0
        for: 30s
        labels:
          severity: critical
        annotations:
          summary: "neighsyncd netlink socket disconnected"

      # Warning: High error rate
      - alert: NeighsyncdHighErrorRate
        expr: |
          rate(neighsyncd_events_failed_total[5m]) /
          rate(neighsyncd_neighbors_processed_total[5m]) > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "neighsyncd high event failure rate"
          description: "Failure rate: {{ $value | humanizePercentage }}"

      # Warning: High latency
      - alert: NeighsyncdHighLatency
        expr: |
          histogram_quantile(0.99,
            rate(neighsyncd_event_latency_seconds_bucket[5m])
          ) > 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "neighsyncd high event latency"
          description: "p99 latency: {{ $value }}s"

      # Warning: High memory usage
      - alert: NeighsyncdHighMemory
        expr: neighsyncd_memory_bytes > 200 * 1024 * 1024
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "neighsyncd high memory usage"
          description: "Memory: {{ $value | humanize }}B"

      # Warning: Queue backlog
      - alert: NeighsyncdQueueBacklog
        expr: neighsyncd_queue_depth > 500
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "neighsyncd event queue backlog"
          description: "Queue depth: {{ $value }}"
```

Load alerts in Prometheus:
```yaml
# prometheus.yml
rule_files:
  - 'neighsyncd_alerts.yml'
```

## Troubleshooting

### Metrics Endpoint Not Accessible

**Symptom**: `curl: (7) Failed to connect to [::1]:9091`

**Causes**:
1. Metrics server not started
2. Wrong port number
3. IPv6 not enabled

**Solutions**:
```bash
# Check if server is running
ss -tlnp | grep 9091

# Check neighsyncd logs
journalctl -u neighsyncd -f

# Verify IPv6 localhost
ping6 ::1

# Try with full client cert authentication
curl --cert client-cert.pem --key client-key.pem --cacert ca-cert.pem \
  https://[::1]:9091/health
```

### mTLS Certificate Errors

**Symptom**: `curl: (35) error:14094412:SSL routines:ssl3_read_bytes:sslv3 alert bad certificate`

**Causes**:
1. Client certificate not signed by configured CA
2. Client certificate expired
3. Client certificate missing required extensions

**Solutions**:
```bash
# Verify client certificate is valid
openssl x509 -in client-cert.pem -noout -text

# Check expiration
openssl x509 -in client-cert.pem -noout -enddate

# Verify certificate chain
openssl verify -CAfile ca-cert.pem client-cert.pem

# Check server logs for specific error
journalctl -u neighsyncd | grep -i "certificate\|tls"
```

### High Error Rates

**Symptom**: `neighsyncd_events_failed_total` increasing rapidly

**Causes**:
1. Redis connection failures
2. Invalid neighbor entries from kernel
3. Database write errors

**Solutions**:
```bash
# Check Redis connectivity
redis-cli -h localhost -p 6379 PING

# Check neighsyncd error logs
journalctl -u neighsyncd | grep -i "error\|failed"

# Monitor Redis latency
redis-cli --latency

# Check metrics for specific error types
curl --cert client-cert.pem --key client-key.pem --cacert ca-cert.pem \
  https://[::1]:9091/metrics | grep error
```

### High Latency

**Symptom**: `histogram_quantile(0.99, rate(neighsyncd_event_latency_seconds_bucket[5m])) > 0.1`

**Causes**:
1. Redis slow queries
2. Network congestion
3. CPU saturation
4. Large batch sizes

**Solutions**:
```bash
# Check Redis latency
redis-cli --latency-history

# Monitor CPU usage
top -p $(pidof neighsyncd)

# Check batch size distribution
curl --cert client-cert.pem --key client-key.pem --cacert ca-cert.pem \
  https://[::1]:9091/metrics | grep batch_size

# Enable performance features
cargo build --release --features perf-all
```

### Memory Leaks

**Symptom**: `neighsyncd_memory_bytes` continuously increasing

**Causes**:
1. Event queue not draining
2. Pending neighbors accumulating
3. Actual memory leak

**Solutions**:
```bash
# Check queue depth
curl --cert client-cert.pem --key client-key.pem --cacert ca-cert.pem \
  https://[::1]:9091/metrics | grep queue_depth

# Check pending neighbors
curl --cert client-cert.pem --key client-key.pem --cacert ca-cert.pem \
  https://[::1]:9091/metrics | grep pending_neighbors

# Monitor with valgrind (development only)
valgrind --leak-check=full ./target/debug/neighsyncd
```

### Health Status Degraded

**Symptom**: `neighsyncd_health_status == 0.5`

**Causes**:
1. Event failure rate > 5%
2. Recent errors recovering

**Solutions**:
```bash
# Check failure rate
curl --cert client-cert.pem --key client-key.pem --cacert ca-cert.pem \
  https://[::1]:9091/metrics | grep failed_total

# Check recent errors in logs
journalctl -u neighsyncd --since "5 minutes ago" | grep -i error

# Check health endpoint for details
curl --cert client-cert.pem --key client-key.pem --cacert ca-cert.pem \
  https://[::1]:9091/health
```

## NIST 800-53 Rev 5 Control Mappings

The monitoring infrastructure implements the following security controls:

| Control | Description | Implementation |
|---------|-------------|----------------|
| **AU-6** | Audit Record Review | Metrics endpoint for analysis tools |
| **AU-12** | Audit Record Generation | All events tracked via metrics |
| **SI-4** | System Monitoring | Real-time performance and health metrics |
| **CP-10** | System Recovery | Health status tracking during recovery |
| **SC-8** | Transmission Confidentiality | TLS 1.3 encryption (mTLS) |
| **SC-8(1)** | Cryptographic Protection | CNSA 2.0 cipher suites |
| **SC-13** | Cryptographic Protection | FIPS 140-3 validated crypto (AWS-LC-RS) |
| **IA-5(2)** | PKI-Based Authentication | Client certificate validation |

## See Also

- [SECURITY.md](SECURITY.md) - CNSA 2.0 compliance details and security architecture
- [DEPLOYMENT.md](DEPLOYMENT.md) - Production deployment procedures
- [TROUBLESHOOTING.md](TROUBLESHOOTING.md) - Detailed troubleshooting guide
