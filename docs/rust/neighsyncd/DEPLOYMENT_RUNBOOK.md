# neighsyncd Deployment Runbook

**Version:** 1.0
**Date:** January 25, 2026
**Status:** Production-Ready

---

## Quick Reference

| What | Command |
|------|---------|
| **Check status** | `systemctl status neighsyncd` |
| **View logs** | `journalctl -u neighsyncd -f` |
| **Check health** | `curl http://[::1]:9091/health` |
| **View metrics** | `curl http://[::1]:9091/metrics` |
| **Restart service** | `systemctl restart neighsyncd` |
| **Stop service** | `systemctl stop neighsyncd` |

---

## Pre-Deployment Checklist

### System Requirements ✅

- [ ] Linux kernel 4.15+ installed
- [ ] Redis running on port 6379
- [ ] Sufficient disk space (1+ GB for logs)
- [ ] Network connectivity to Redis
- [ ] Root/sudo access available

**Verify:**
```bash
# Check kernel version
uname -r  # Should be >= 4.15

# Check Redis
redis-cli ping  # Should return PONG

# Check disk space
df -h /var/log  # Should have > 1GB free

# Check connectivity
ping -c 1 127.0.0.1  # Should succeed
```

### Binary Requirements ✅

- [ ] Rust 1.85+ installed (for building)
- [ ] Source code checked out
- [ ] All dependencies available

**Verify:**
```bash
# Check Rust version
rustc --version  # Should be >= 1.85

# Check source code
cd /path/to/sonic-swss
ls crates/neighsyncd/src/main.rs  # Should exist

# Run tests
cargo test --lib -p sonic-neighsyncd  # Should pass 126/126
```

---

## Deployment Procedure

### Step 1: Build Release Binary (5-10 minutes)

```bash
cd /path/to/sonic-swss

# Build release binary
cargo build --release -p sonic-neighsyncd

# Verify binary created
ls -lh target/release/neighsyncd
# Expected: Binary ~8-10 MB

# Quick sanity check
./target/release/neighsyncd --version
# Expected: Version info displayed
```

**Troubleshooting:**
- If build fails: Check Rust version and dependencies
- If binary missing: Check build output for errors
- If too slow: Use `--jobs 4` to parallelize build

### Step 2: Install Binary (2 minutes)

```bash
# Copy binary to system location
sudo cp target/release/neighsyncd /usr/local/bin/
sudo chmod 755 /usr/local/bin/neighsyncd

# Verify installation
which neighsyncd  # Should show /usr/local/bin/neighsyncd
neighsyncd --version  # Should display version
```

### Step 3: Create Directories (1 minute)

```bash
# Create service directories
sudo mkdir -p /etc/neighsyncd
sudo mkdir -p /var/lib/neighsyncd
sudo mkdir -p /var/log/neighsyncd

# Set permissions
sudo chown root:root /etc/neighsyncd
sudo chown root:root /var/lib/neighsyncd
sudo chown syslog:adm /var/log/neighsyncd

sudo chmod 755 /etc/neighsyncd
sudo chmod 755 /var/lib/neighsyncd
sudo chmod 755 /var/log/neighsyncd

# Verify
ls -ld /etc/neighsyncd /var/lib/neighsyncd /var/log/neighsyncd
```

### Step 4: Install Configuration (2 minutes)

```bash
# Copy example configuration
sudo cp crates/neighsyncd/neighsyncd.conf.example /etc/neighsyncd/neighsyncd.conf

# Edit configuration for your environment
sudo nano /etc/neighsyncd/neighsyncd.conf
```

**Minimum Configuration:**
```toml
[redis]
host = "127.0.0.1"
port = 6379
database = 0  # APPL_DB

[logging]
level = "info"
format = "json"

[monitoring]
metrics_enabled = true
metrics_port = 9091
```

**Verify configuration:**
```bash
# Check syntax (neighsyncd will validate on start)
cat /etc/neighsyncd/neighsyncd.conf
```

### Step 5: Install Systemd Service (2 minutes)

```bash
# Copy systemd unit file
sudo cp crates/neighsyncd/neighsyncd.service /etc/systemd/system/

# Reload systemd
sudo systemctl daemon-reload

# Enable service (start on boot)
sudo systemctl enable neighsyncd

# Verify
systemctl is-enabled neighsyncd  # Should show "enabled"
```

### Step 6: Start Service (1 minute)

```bash
# Start the service
sudo systemctl start neighsyncd

# Check status
sudo systemctl status neighsyncd
# Expected: Active (running)

# Check logs
journalctl -u neighsyncd -n 50
# Expected: No errors, startup messages
```

### Step 7: Verify Operation (2 minutes)

```bash
# 1. Check health endpoint
curl http://[::1]:9091/health
# Expected: {"status":"healthy",...}

# 2. Check metrics endpoint
curl http://[::1]:9091/metrics | head -20
# Expected: Prometheus metrics output

# 3. Check Redis connectivity
redis-cli -n 0 KEYS "NEIGH_TABLE:*"
# Expected: List of neighbor keys (may be empty initially)

# 4. Check logs for errors
journalctl -u neighsyncd --since "5 minutes ago" | grep -i error
# Expected: No errors
```

---

## Post-Deployment Validation

### Health Check Script

```bash
#!/bin/bash
# File: /usr/local/bin/neighsyncd-health-check.sh

echo "=== neighsyncd Health Check ==="

# 1. Service status
echo -n "Service Status: "
systemctl is-active neighsyncd || echo "FAILED"

# 2. Health endpoint
echo -n "Health Endpoint: "
curl -s http://[::1]:9091/health | jq -r '.status' || echo "FAILED"

# 3. Metrics endpoint
echo -n "Metrics Endpoint: "
curl -s http://[::1]:9091/metrics > /dev/null && echo "OK" || echo "FAILED"

# 4. Redis connectivity
echo -n "Redis Connection: "
redis-cli ping > /dev/null && echo "OK" || echo "FAILED"

# 5. Recent errors
echo -n "Recent Errors: "
ERROR_COUNT=$(journalctl -u neighsyncd --since "5 minutes ago" | grep -c -i error)
if [ "$ERROR_COUNT" -eq 0 ]; then
    echo "None"
else
    echo "$ERROR_COUNT found"
fi

echo "=== End Health Check ==="
```

**Usage:**
```bash
chmod +x /usr/local/bin/neighsyncd-health-check.sh
/usr/local/bin/neighsyncd-health-check.sh
```

### Monitoring Setup

**1. Prometheus Scrape Configuration:**
```yaml
# Add to prometheus.yml
scrape_configs:
  - job_name: 'neighsyncd'
    static_configs:
      - targets: ['localhost:9091']
```

**2. Grafana Dashboard:**
```bash
# Import dashboard
# File: crates/neighsyncd/dashboards/neighsyncd.json
# Import via Grafana UI: Dashboards → Import → Upload JSON
```

**3. Alert Rules:**
```bash
# Copy alert rules
sudo cp crates/neighsyncd/alerts.yaml /etc/prometheus/rules/neighsyncd.yaml

# Reload Prometheus
sudo systemctl reload prometheus
```

---

## Common Operations

### Restart Service

```bash
# Graceful restart
sudo systemctl restart neighsyncd

# Check status after restart
systemctl status neighsyncd
journalctl -u neighsyncd -n 20

# Verify health
curl http://[::1]:9091/health
```

### View Logs

```bash
# Real-time logs
journalctl -u neighsyncd -f

# Last 100 lines
journalctl -u neighsyncd -n 100

# Logs since specific time
journalctl -u neighsyncd --since "1 hour ago"

# Only errors
journalctl -u neighsyncd -p err

# Export logs
journalctl -u neighsyncd --since today > /tmp/neighsyncd-$(date +%Y%m%d).log
```

### Update Configuration

```bash
# 1. Edit configuration
sudo nano /etc/neighsyncd/neighsyncd.conf

# 2. Validate changes (optional)
# Review the file for syntax errors

# 3. Reload service
sudo systemctl reload neighsyncd
# OR
sudo systemctl restart neighsyncd

# 4. Verify
journalctl -u neighsyncd -n 20  # Check for config load messages
```

### Update Binary

```bash
# 1. Build new version
cd /path/to/sonic-swss
git pull
cargo build --release -p sonic-neighsyncd

# 2. Stop service
sudo systemctl stop neighsyncd

# 3. Backup old binary
sudo cp /usr/local/bin/neighsyncd /usr/local/bin/neighsyncd.backup

# 4. Install new binary
sudo cp target/release/neighsyncd /usr/local/bin/

# 5. Start service
sudo systemctl start neighsyncd

# 6. Verify
systemctl status neighsyncd
curl http://[::1]:9091/health
```

---

## Troubleshooting Guide

### Issue: Service Won't Start

**Symptoms:**
```bash
$ systemctl status neighsyncd
● neighsyncd.service - Neighbor Synchronization Daemon
   Loaded: loaded
   Active: failed
```

**Diagnosis:**
```bash
# Check logs
journalctl -u neighsyncd -n 50

# Check configuration
sudo neighsyncd --validate-config /etc/neighsyncd/neighsyncd.conf

# Check permissions
ls -la /usr/local/bin/neighsyncd
ls -la /etc/neighsyncd/neighsyncd.conf
```

**Solutions:**
1. Configuration error → Fix syntax in neighsyncd.conf
2. Permission denied → Check binary and config permissions
3. Port already in use → Check if another instance is running

### Issue: High Error Rate

**Symptoms:**
```bash
$ curl http://[::1]:9091/metrics | grep error
neighsyncd_events_failed_total 150
```

**Diagnosis:**
```bash
# Check recent errors
journalctl -u neighsyncd --since "10 minutes ago" | grep ERROR

# Check Redis connection
redis-cli ping

# Check network latency
redis-cli --latency

# Check metrics
curl http://[::1]:9091/metrics | grep -E "(error|failed)"
```

**Solutions:**
1. Redis unavailable → Restart Redis, check network
2. High latency → Check Redis performance, network congestion
3. Invalid data → Check netlink messages in logs

### Issue: High Memory Usage

**Symptoms:**
```bash
$ curl http://[::1]:9091/metrics | grep memory
neighsyncd_memory_bytes 524288000  # > 500 MB
```

**Diagnosis:**
```bash
# Check neighbor count
curl http://[::1]:9091/health | jq '.neighbors_count'

# Check process memory
ps aux | grep neighsyncd

# Check for memory leaks
# Run for extended period and monitor growth
```

**Solutions:**
1. Too many neighbors → Expected, adjust resources
2. Memory leak → Restart service, report bug if persists
3. Configuration issue → Check batch_size settings

### Issue: High Latency

**Symptoms:**
```bash
$ curl http://[::1]:9091/metrics | grep latency
neighsyncd_event_latency_seconds_bucket{le="0.1"} 50  # Many events > 100ms
```

**Diagnosis:**
```bash
# Check Redis latency
redis-cli --latency

# Check system load
top
iostat

# Check network
ping 127.0.0.1
```

**Solutions:**
1. Redis slow → Optimize Redis, check disk I/O
2. High system load → Add resources, reduce load
3. Large batch size → Tune batch_size in config

---

## Performance Tuning Quick Reference

### For Small Networks (< 1,000 neighbors)

```toml
[performance]
batch_size = 50
worker_threads = 1
auto_tune_enabled = false
```

### For Medium Networks (1,000 - 10,000 neighbors)

```toml
[performance]
batch_size = 100
worker_threads = 4
auto_tune_enabled = true
tuning_strategy = "Balanced"
```

### For Large Networks (10,000+ neighbors)

```toml
[performance]
batch_size = 500
worker_threads = 8
auto_tune_enabled = true
tuning_strategy = "Aggressive"
```

---

## Rollback Procedure

### If New Version Has Issues

```bash
# 1. Stop new version
sudo systemctl stop neighsyncd

# 2. Restore old binary
sudo cp /usr/local/bin/neighsyncd.backup /usr/local/bin/neighsyncd

# 3. Restore old config (if changed)
sudo cp /etc/neighsyncd/neighsyncd.conf.backup /etc/neighsyncd/neighsyncd.conf

# 4. Start old version
sudo systemctl start neighsyncd

# 5. Verify
systemctl status neighsyncd
curl http://[::1]:9091/health
```

---

## Emergency Procedures

### Complete Service Failure

```bash
# 1. Stop service
sudo systemctl stop neighsyncd

# 2. Check Redis
redis-cli ping

# 3. Clear any locks (if using HA)
redis-cli -n 4 KEYS "*neighsyncd*lock*"
redis-cli -n 4 DEL <lock_key>

# 4. Start service
sudo systemctl start neighsyncd

# 5. Monitor closely
journalctl -u neighsyncd -f
```

### Data Corruption

```bash
# 1. Stop service
sudo systemctl stop neighsyncd

# 2. Backup current state
redis-cli -n 0 BGSAVE
sudo cp /var/lib/redis/dump.rdb /backup/redis-$(date +%Y%m%d-%H%M%S).rdb

# 3. Clear neighbor table (DANGER!)
redis-cli -n 0 KEYS "NEIGH_TABLE:*" | xargs redis-cli -n 0 DEL

# 4. Start service (will repopulate from kernel)
sudo systemctl start neighsyncd

# 5. Verify repopulation
watch -n 1 'redis-cli -n 0 KEYS "NEIGH_TABLE:*" | wc -l'
```

---

## Maintenance Windows

### Planned Downtime Procedure

```bash
# Before maintenance window
# 1. Announce to monitoring team
# 2. Disable alerts for this host

# During maintenance
sudo systemctl stop neighsyncd
# Perform maintenance (updates, config changes, etc.)
sudo systemctl start neighsyncd

# After maintenance
curl http://[::1]:9091/health  # Verify health
systemctl status neighsyncd     # Verify running
# 3. Re-enable alerts
# 4. Announce completion
```

---

## Checklist Summary

### ✅ Pre-Deployment
- [ ] System requirements verified
- [ ] Redis running and accessible
- [ ] Binary built and tested
- [ ] Configuration prepared

### ✅ Deployment
- [ ] Binary installed to /usr/local/bin
- [ ] Directories created with correct permissions
- [ ] Configuration installed
- [ ] Systemd service installed and enabled
- [ ] Service started successfully

### ✅ Post-Deployment
- [ ] Health endpoint responding
- [ ] Metrics endpoint responding
- [ ] Redis connectivity confirmed
- [ ] No errors in logs
- [ ] Monitoring configured
- [ ] Alerts configured

### ✅ Ongoing
- [ ] Monitor metrics daily
- [ ] Review logs weekly
- [ ] Update binary monthly (or as needed)
- [ ] Test failover quarterly

---

## Support Contacts

**Documentation:**
- Architecture: `ARCHITECTURE.md`
- Configuration: `CONFIGURATION.md`
- Troubleshooting: `TROUBLESHOOTING.md`
- Performance: `NEIGHSYNCD_PERFORMANCE_BASELINES.md`

**For Issues:**
1. Check logs: `journalctl -u neighsyncd`
2. Check health: `curl http://[::1]:9091/health`
3. Review documentation
4. Escalate to development team if unresolved

---

**Runbook Version:** 1.0
**Last Updated:** January 25, 2026
**Status:** Production-Ready
