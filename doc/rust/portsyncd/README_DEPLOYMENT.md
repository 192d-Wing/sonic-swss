# SONiC Port Synchronization Daemon - Deployment Guide

**Phase 6 Week 4 Deployment Documentation**

This guide covers the deployment of the Rust implementation of portsyncd with advanced metrics persistence, monitoring, and health checking capabilities.

## Table of Contents

1. [Overview](#overview)
2. [System Requirements](#system-requirements)
3. [Pre-Deployment Checklist](#pre-deployment-checklist)
4. [Installation Steps](#installation-steps)
5. [Configuration](#configuration)
6. [Systemd Integration](#systemd-integration)
7. [Metrics Persistence](#metrics-persistence)
8. [Monitoring & Alerting](#monitoring--alerting)
9. [Troubleshooting](#troubleshooting)
10. [Performance Tuning](#performance-tuning)
11. [Backup & Recovery](#backup--recovery)
12. [Upgrade Path](#upgrade-path)

---

## Overview

The Rust portsyncd daemon provides:

- **Real-time port synchronization** - Synchronizes kernel port status with SONiC databases
- **Metrics persistence** - Stores warm restart and health metrics across daemon restarts
- **Prometheus export** - Exports metrics in Prometheus format for monitoring/alerting
- **Health monitoring** - Continuous health assessment with systemd watchdog integration
- **Advanced configuration** - TOML-based configuration with validation
- **Production-ready** - TLS support, security hardening, graceful shutdown

### Key Files

| File | Purpose |
|------|---------|
| `portsyncd` | Binary executable |
| `portsyncd.service` | Systemd unit file |
| `portsyncd.conf.example` | Configuration template |
| `README_DEPLOYMENT.md` | This file |

---

## System Requirements

### Hardware

- **CPU**: Any modern x86-64 or ARM64 processor
- **Memory**: Minimum 512 MB, recommended 1 GB
- **Disk**: 500 MB for binary + config + metrics storage

### Software

- **OS**: SONiC Linux (based on Debian)
- **systemd**: Version 200+ (for Type=notify)
- **Redis**: 3.2+ (for CONFIG_DB and STATE_DB)
- **Kernel**: 4.4+ (for netlink socket support)

### Network

- **Redis connectivity**: Must reach Redis server on configured host:port
- **Optional**: HTTP access to metrics endpoint (default: [::1]:9090 IPv6)

---

## Pre-Deployment Checklist

Before installing portsyncd, verify:

- [ ] Redis server is running and accessible

  ```bash
  redis-cli ping
  # Expected: PONG
  ```

- [ ] Kernel supports netlink routes (NETLINK_ROUTE)

  ```bash
  grep NETLINK_ROUTE /boot/config-$(uname -r)
  # Expected: CONFIG_NETLINK_ROUTE=y
  ```

- [ ] Required directories are writable:

  ```bash
  ls -la /var/lib/sonic/portsyncd/
  # Expected: drwxr-xr-x portsyncd portsyncd
  ```

- [ ] No other portsyncd is running:

  ```bash
  systemctl status portsyncd
  # Expected: inactive (dead)
  ```

- [ ] Network connectivity to target ports is available:

  ```bash
  ip link show | grep -E "Ethernet|etp"
  # Expected: List of ports
  ```

---

## Installation Steps

### 1. Build from Source

```bash
cd sonic-swss/crates/portsyncd
cargo build --release
cargo test --all-features  # Run tests
```

### 2. Install Binary

```bash
# Copy binary to system location
sudo cp target/release/portsyncd /usr/local/bin/
sudo chown root:root /usr/local/bin/portsyncd
sudo chmod 755 /usr/local/bin/portsyncd

# Verify installation
/usr/local/bin/portsyncd --version
```

### 3. Install Systemd Service

```bash
# Copy systemd unit file
sudo cp portsyncd.service /etc/systemd/system/
sudo chown root:root /etc/systemd/system/portsyncd.service
sudo chmod 644 /etc/systemd/system/portsyncd.service

# Reload systemd daemon
sudo systemctl daemon-reload
```

### 4. Create Configuration

```bash
# Copy example configuration
sudo cp portsyncd.conf.example /etc/sonic/portsyncd.conf
sudo chown root:root /etc/sonic/portsyncd.conf
sudo chmod 644 /etc/sonic/portsyncd.conf

# Edit for your environment
sudo nano /etc/sonic/portsyncd.conf
```

### 5. Create Metrics Directory

```bash
# Create metrics storage directory
sudo mkdir -p /var/lib/sonic/portsyncd/metrics
sudo mkdir -p /var/lib/sonic/portsyncd/backups

# Set permissions
sudo chown portsyncd:portsyncd /var/lib/sonic/portsyncd
sudo chown portsyncd:portsyncd /var/lib/sonic/portsyncd/metrics
sudo chown portsyncd:portsyncd /var/lib/sonic/portsyncd/backups
sudo chmod 750 /var/lib/sonic/portsyncd
sudo chmod 750 /var/lib/sonic/portsyncd/metrics
sudo chmod 750 /var/lib/sonic/portsyncd/backups
```

### 6. Create System User (if needed)

```bash
# Create portsyncd user for daemon execution
sudo useradd -r -s /bin/false -d /var/lib/sonic/portsyncd portsyncd

# Add to necessary groups
sudo usermod -aG redis portsyncd  # If Redis requires group access
```

### 7. Enable and Start Service

```bash
# Enable service to start on boot
sudo systemctl enable portsyncd

# Start service
sudo systemctl start portsyncd

# Verify service is running
sudo systemctl status portsyncd

# Check logs
sudo journalctl -u portsyncd -f
```

---

## Configuration

### Configuration File Location

- **File**: `/etc/sonic/portsyncd.conf`
- **Format**: TOML
- **Ownership**: `root:root`
- **Permissions**: `644` (world-readable)

### Configuration Sections

#### Database Section

```toml
[database]
redis_host = "127.0.0.1"
redis_port = 6379
config_db_number = 4
state_db_number = 6
connection_timeout_secs = 5
retry_interval_secs = 2
```

**Description**: Redis connection parameters for SONiC databases.

#### Performance Section

```toml
[performance]
max_event_queue = 1000
batch_timeout_ms = 100
max_latency_us = 10000
min_success_rate = 99.0
```

**Description**: Event processing and latency targets.

#### Health Section

```toml
[health]
max_stall_seconds = 10
max_failure_rate_percent = 5.0
min_port_sync_rate = 90.0
enable_watchdog = true
watchdog_interval_secs = 15
```

**Description**: Health monitoring thresholds.

#### Metrics Section

```toml
[metrics]
enabled = true
save_interval_secs = 300
retention_days = 30
max_file_size_mb = 100
export_format = "prometheus"
storage_path = "/var/lib/sonic/portsyncd/metrics"
```

**Description**: Metrics persistence and export configuration (Phase 6 Week 4).

### Validation

All configuration values are validated on startup:

```bash
# Test configuration without starting daemon
/usr/local/bin/portsyncd --validate-config

# Expected output on success:
# Configuration valid
```

---

## Systemd Integration

### Service Status

```bash
# Check current status
systemctl status portsyncd

# Check if enabled for auto-start
systemctl is-enabled portsyncd

# View recent logs
journalctl -u portsyncd -n 50

# Tail logs in real-time
journalctl -u portsyncd -f
```

### Service Control

```bash
# Start service
sudo systemctl start portsyncd

# Stop service gracefully
sudo systemctl stop portsyncd

# Restart service
sudo systemctl restart portsyncd

# Reload configuration (without restart)
sudo systemctl reload portsyncd

# Enable/disable auto-start
sudo systemctl enable portsyncd
sudo systemctl disable portsyncd
```

### Watchdog Monitoring

The daemon sends watchdog notifications to systemd every 15 seconds (configurable).

```bash
# Check watchdog interval in service file
grep WatchdogSec /etc/systemd/system/portsyncd.service
# Expected: WatchdogSec=30s

# Systemd will restart daemon if no watchdog signal for 30 seconds
# This indicates the daemon has stalled
```

---

## Metrics Persistence

### Metrics Files

Metrics are stored in JSON format at:

```
/var/lib/sonic/portsyncd/metrics/
├── metrics.json                    # Current metrics
├── backups/
│   ├── port_state_1609459200.json # Timestamped backups
│   ├── port_state_1609459300.json
│   └── ... (up to 10 by default)
```

### Metrics Lifecycle

1. **Recording**: Metrics are recorded in-memory as events occur
2. **Auto-save**: Metrics are persisted to JSON every `save_interval_secs` (default: 300)
3. **Rotation**: When metrics file exceeds `max_file_size_mb`, old backups are created
4. **Cleanup**: Metrics older than `retention_days` (default: 30) are removed
5. **Recovery**: On daemon restart, metrics are loaded from persistent storage

### Metrics Exported

**Counters**:

- `portsyncd_warm_restarts` - Warm restart events
- `portsyncd_cold_starts` - Cold start events
- `portsyncd_eoiu_detected` - EOIU signals received
- `portsyncd_eoiu_timeouts` - EOIU auto-completion timeouts
- `portsyncd_state_recoveries` - Successful state recoveries
- `portsyncd_corruptions_detected` - Corruption events
- `portsyncd_backups_created` - Backup files created
- `portsyncd_backups_cleaned` - Backup files cleaned

**Gauges** (timestamps):

- `portsyncd_last_warm_restart_timestamp`
- `portsyncd_last_eoiu_detection_timestamp`
- `portsyncd_last_state_recovery_timestamp`
- `portsyncd_last_corruption_timestamp`

**Histograms**:

- `portsyncd_initial_sync_duration_seconds` - Initial sync latency distribution

### Accessing Metrics

#### JSON Format

```bash
cat /var/lib/sonic/portsyncd/metrics/metrics.json
```

**Output**:

```json
{
  "warm_restarts": 5,
  "cold_starts": 1,
  "eoiu_detected": 4,
  "eoiu_timeouts": 1,
  "state_recoveries": 2,
  "corruptions_detected": 1,
  "backups_created": 6,
  "backups_cleaned": 2,
  "avg_initial_sync_duration_secs": 5.5,
  ...
}
```

#### Prometheus Format (HTTP)

```bash
curl -s http://127.0.0.1:9090/metrics
```

**Output**:

```
# HELP portsyncd_warm_restarts Total warm restart events
# TYPE portsyncd_warm_restarts counter
portsyncd_warm_restarts 5

# HELP portsyncd_eoiu_detected Total EOIU signals detected
# TYPE portsyncd_eoiu_detected counter
portsyncd_eoiu_detected 4
...
```

---

## Monitoring & Alerting

### Prometheus Integration

#### Prometheus Configuration

Add to `/etc/prometheus/prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'portsyncd'
    static_configs:
      - targets: ['127.0.0.1:9090']
    scrape_interval: 15s
    scrape_timeout: 10s
```

#### Grafana Dashboard

Sample dashboard JSON queries:

```
# Warm restart percentage
rate(portsyncd_warm_restarts[5m]) / (rate(portsyncd_warm_restarts[5m]) + rate(portsyncd_cold_starts[5m]))

# System health score (requires custom exporter)
portsyncd_health_score

# EOIU timeout rate
rate(portsyncd_eoiu_timeouts[5m]) / rate(portsyncd_eoiu_detected[5m])
```

### Alert Rules

```yaml
# PrometheusRules for portsyncd monitoring

groups:
  - name: portsyncd.rules
    interval: 30s
    rules:
      - alert: PortSyncdDown
        expr: up{job="portsyncd"} == 0
        for: 1m
        annotations:
          summary: "portsyncd is down"

      - alert: HighEOIUTimeoutRate
        expr: rate(portsyncd_eoiu_timeouts[5m]) / rate(portsyncd_eoiu_detected[5m]) > 0.5
        for: 5m
        annotations:
          summary: "EOIU timeout rate > 50%"

      - alert: CorruptionNotRecovered
        expr: portsyncd_corruptions_detected > portsyncd_state_recoveries
        for: 2m
        annotations:
          summary: "Unrecovered corruption detected"

      - alert: HighColdStartRate
        expr: rate(portsyncd_cold_starts[5m]) > 0.1
        for: 5m
        annotations:
          summary: "Cold start rate > 0.1/sec"
```

### Logging

Logs are sent to systemd journal:

```bash
# View all portsyncd logs
journalctl -u portsyncd

# View only errors
journalctl -u portsyncd -p err

# View logs from last hour
journalctl -u portsyncd --since "1 hour ago"

# Follow logs in real-time
journalctl -u portsyncd -f

# View logs with JSON formatting
journalctl -u portsyncd -o json
```

---

## Troubleshooting

### Service Won't Start

**Symptom**: `systemctl start portsyncd` fails

**Diagnosis**:

```bash
journalctl -u portsyncd -n 50  # Check logs
systemctl status portsyncd     # Check status
```

**Common Issues**:

1. **Configuration error**:

   ```bash
   # Validate configuration
   /usr/local/bin/portsyncd --validate-config

   # Check syntax
   cat /etc/sonic/portsyncd.conf
   ```

2. **Redis connection failure**:

   ```bash
   # Verify Redis is running
   redis-cli ping

   # Check connectivity
   redis-cli -h <redis_host> -p <redis_port> ping
   ```

3. **Permission denied**:

   ```bash
   # Check metrics directory permissions
   ls -la /var/lib/sonic/portsyncd/metrics/

   # Fix permissions
   sudo chown portsyncd:portsyncd /var/lib/sonic/portsyncd/metrics
   ```

### High CPU Usage

**Symptom**: portsyncd process using > 50% CPU

**Diagnosis**:

```bash
# Profile CPU usage
top -p $(pgrep portsyncd)

# Check event queue depth
grep portsyncd_queue_depth /var/lib/sonic/portsyncd/metrics/metrics.json
```

**Solutions**:

1. Increase batch timeout (processes fewer events per second):

   ```toml
   [performance]
   batch_timeout_ms = 200  # Default: 100
   ```

2. Reduce metrics save frequency (less disk I/O):

   ```toml
   [metrics]
   save_interval_secs = 600  # Default: 300
   ```

### High Memory Usage

**Symptom**: portsyncd process using > 200 MB RAM

**Diagnosis**:

```bash
# Check actual memory usage
ps aux | grep portsyncd

# Check metrics file size
du -sh /var/lib/sonic/portsyncd/metrics/
```

**Solutions**:

1. Reduce metrics retention:

   ```toml
   [metrics]
   retention_days = 7  # Default: 30
   ```

2. Reduce metrics file size limit:

   ```toml
   [metrics]
   max_file_size_mb = 50  # Default: 100
   ```

### Metrics Not Being Saved

**Symptom**: `/var/lib/sonic/portsyncd/metrics/metrics.json` not being created/updated

**Diagnosis**:

```bash
# Check if metrics are being recorded
journalctl -u portsyncd | grep -i metrics

# Check directory permissions
ls -la /var/lib/sonic/portsyncd/metrics/

# Check disk space
df -h /var/lib/sonic/portsyncd/
```

**Solutions**:

1. Verify metrics are enabled:

   ```toml
   [metrics]
   enabled = true  # Default
   ```

2. Fix directory permissions:

   ```bash
   sudo chmod 750 /var/lib/sonic/portsyncd/metrics
   sudo chown portsyncd:portsyncd /var/lib/sonic/portsyncd/metrics
   ```

3. Ensure adequate disk space:

   ```bash
   df -h /  # Should have > 1 GB free
   ```

---

## Performance Tuning

### For High Port Count (>50 ports)

```toml
[performance]
max_event_queue = 5000        # Increased queue
batch_timeout_ms = 50         # Faster batching

[metrics]
save_interval_secs = 600      # Save less frequently (10 min)
```

### For Low Latency Requirement (<5ms)

```toml
[performance]
max_latency_us = 5000
batch_timeout_ms = 10

[health]
max_stall_seconds = 5
```

### For Long-Term Metrics Storage

```toml
[metrics]
retention_days = 90           # Store 3 months
max_file_size_mb = 500        # Larger files
storage_path = "/mnt/large-disk/portsyncd/metrics"
```

### For Debugging/Development

```toml
[metrics]
save_interval_secs = 60       # Save every minute
export_format = "both"        # Export Prometheus and JSON
enabled = true
```

---

## Backup & Recovery

### Backup Metrics

```bash
# Manual backup of metrics
sudo tar czf portsyncd-metrics-backup.tar.gz \
  /var/lib/sonic/portsyncd/metrics/

# Store off-device
scp portsyncd-metrics-backup.tar.gz user@backup-server:/backup/
```

### Recover Metrics

```bash
# Restore from backup
sudo tar xzf portsyncd-metrics-backup.tar.gz -C /

# Verify restoration
ls -la /var/lib/sonic/portsyncd/metrics/

# Restart daemon to load restored metrics
sudo systemctl restart portsyncd
```

### State File Recovery

If corruption is detected, the daemon automatically uses backup state files:

```bash
# Check backups available
ls -la /var/lib/sonic/portsyncd/backups/

# Manually inspect backup
cat /var/lib/sonic/portsyncd/backups/port_state_*.json

# Daemon will automatically try newest backup first
```

---

## Upgrade Path

### From C++ portsyncd to Rust portsyncd

1. **Backup existing configuration**:

   ```bash
   sudo cp /etc/sonic/portsyncd.conf /etc/sonic/portsyncd.conf.backup
   ```

2. **Stop old daemon**:

   ```bash
   sudo systemctl stop portsyncd
   ```

3. **Install new binary** (follow Installation Steps above)

4. **Migrate configuration**:

   ```bash
   # Rust version uses same format, verify compatibility
   /usr/local/bin/portsyncd --validate-config
   ```

5. **Start new daemon**:

   ```bash
   sudo systemctl start portsyncd
   ```

6. **Verify functionality**:

   ```bash
   # Check daemon is running
   systemctl status portsyncd

   # Verify port synchronization
   redis-cli -n 6 HGETALL "PORT|Ethernet0"

   # Check metrics
   cat /var/lib/sonic/portsyncd/metrics/metrics.json
   ```

### Rollback Procedure

If issues occur:

```bash
# Stop new daemon
sudo systemctl stop portsyncd

# Restore old binary
sudo cp /usr/local/bin/portsyncd.backup /usr/local/bin/portsyncd

# Restore old configuration
sudo cp /etc/sonic/portsyncd.conf.backup /etc/sonic/portsyncd.conf

# Start old daemon
sudo systemctl restart portsyncd
```

---

## Support & Maintenance

### Version Information

```bash
/usr/local/bin/portsyncd --version
```

### Health Checks

Regular maintenance tasks:

1. **Weekly**: Review metrics for anomalies

   ```bash
   cat /var/lib/sonic/portsyncd/metrics/metrics.json | jq '.health_score'
   ```

2. **Monthly**: Verify disk usage

   ```bash
   du -sh /var/lib/sonic/portsyncd/metrics/
   ```

3. **Quarterly**: Review logs for errors

   ```bash
   journalctl -u portsyncd --since "3 months ago" | grep -i error
   ```

### Reporting Issues

Include the following in bug reports:

1. Configuration file (sanitized)
2. Recent logs: `journalctl -u portsyncd -n 500`
3. Metrics dump: `cat /var/lib/sonic/portsyncd/metrics/metrics.json`
4. System info: `uname -a`, `redis-cli info`, `df -h`

---

## Appendix: Quick Reference

### Key Directories

| Directory | Purpose | Owner |
|-----------|---------|-------|
| `/etc/sonic/` | Configuration | root |
| `/var/lib/sonic/portsyncd/` | Runtime state | portsyncd |
| `/var/lib/sonic/portsyncd/metrics/` | Metrics storage | portsyncd |
| `/usr/local/bin/` | Binary | root |
| `/etc/systemd/system/` | Service unit | root |

### Key Files

| File | Purpose | Permissions |
|------|---------|-------------|
| `portsyncd.conf` | Configuration | 644 (rw-r--r--) |
| `metrics.json` | Current metrics | 640 (rw-r-----) |
| `port_state_*.json` | State backups | 640 (rw-r-----) |

### Common Commands

```bash
# Start/stop/status
sudo systemctl start portsyncd
sudo systemctl stop portsyncd
sudo systemctl status portsyncd

# View configuration
cat /etc/sonic/portsyncd.conf

# View metrics
cat /var/lib/sonic/portsyncd/metrics/metrics.json
jq '.warm_restart_count' /var/lib/sonic/portsyncd/metrics/metrics.json

# View logs
journalctl -u portsyncd -f
journalctl -u portsyncd -p err

# Test Redis connectivity
redis-cli ping
redis-cli -n 6 DBSIZE
```

---

**Document Version**: 1.0
**Last Updated**: Phase 6 Week 4
**Author**: Claude Haiku 4.5
**License**: Same as SONiC project
