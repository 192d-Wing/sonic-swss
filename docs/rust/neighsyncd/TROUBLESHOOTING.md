# neighsyncd Troubleshooting Guide

**Version:** 1.0
**Last Updated:** 2026-01-25

## Table of Contents

1. [Quick Diagnostics](#quick-diagnostics)
2. [Service Won't Start](#service-wont-start)
3. [Redis Connection Issues](#redis-connection-issues)
4. [Netlink Socket Errors](#netlink-socket-errors)
5. [High Memory Usage](#high-memory-usage)
6. [High CPU Usage](#high-cpu-usage)
7. [Metrics Endpoint Issues](#metrics-endpoint-issues)
8. [Warm Restart Problems](#warm-restart-problems)
9. [Performance Issues](#performance-issues)
10. [Log Analysis](#log-analysis)
11. [Debug Mode](#debug-mode)
12. [Common Error Messages](#common-error-messages)
13. [Performance Profiling](#performance-profiling)
14. [Known Issues](#known-issues)
15. [Getting Help](#getting-help)

---

## Quick Diagnostics

### Run Quick Health Check

```bash
#!/bin/bash
# Quick health check script

echo "=== neighsyncd Health Check ==="

# 1. Check if service is running
echo -n "Service status: "
systemctl is-active neighsyncd.service && echo "✓ Running" || echo "✗ Not running"

# 2. Check Redis connectivity
echo -n "Redis connectivity: "
redis-cli -h ::1 -p 6379 PING > /dev/null 2>&1 && echo "✓ OK" || echo "✗ Failed"

# 3. Check metrics endpoint
echo -n "Metrics endpoint: "
curl -k --max-time 2 https://[::1]:9091/health > /dev/null 2>&1 && echo "✓ OK" || echo "✗ Failed"

# 4. Check memory usage
MEM=$(ps aux | grep sonic-neighsyncd | grep -v grep | awk '{print $6}')
echo "Memory usage: ${MEM} KB"

# 5. Check recent errors
ERRORS=$(journalctl -u neighsyncd.service --since "5 minutes ago" | grep -i error | wc -l)
echo "Recent errors (5 min): ${ERRORS}"

# 6. Check neighbor count
KERNEL_COUNT=$(ip -6 neigh show | wc -l)
REDIS_COUNT=$(redis-cli -h ::1 KEYS "NEIGH_TABLE:*" | wc -l)
echo "Kernel neighbors: ${KERNEL_COUNT}"
echo "Redis neighbors: ${REDIS_COUNT}"
echo "Diff: $((KERNEL_COUNT - REDIS_COUNT))"

echo "=== End Health Check ==="
```

Save as `/usr/local/bin/neighsyncd-health.sh`, make executable, and run:

```bash
sudo chmod +x /usr/local/bin/neighsyncd-health.sh
sudo /usr/local/bin/neighsyncd-health.sh
```

---

## Service Won't Start

### Symptom

```bash
$ sudo systemctl start neighsyncd.service
Job for neighsyncd.service failed because the control process exited with error code.
```

### Diagnostic Steps

#### 1. Check systemd status

```bash
sudo systemctl status neighsyncd.service

# Look for error messages in output
```

#### 2. View detailed logs

```bash
# Last 50 lines
sudo journalctl -u neighsyncd.service -n 50

# Follow logs in real-time
sudo journalctl -u neighsyncd.service -f
```

#### 3. Check binary permissions

```bash
ls -l /usr/local/bin/sonic-neighsyncd

# Should be:
# -rwxr-xr-x 1 root root ... /usr/local/bin/sonic-neighsyncd

# Fix if needed:
sudo chmod 0755 /usr/local/bin/sonic-neighsyncd
```

#### 4. Verify configuration file

```bash
# Check if configuration file exists
ls -l /etc/sonic/neighsyncd/neighsyncd.conf

# Validate TOML syntax
sonic-neighsyncd --check-config

# Check permissions (must be readable by sonic user)
sudo chmod 0640 /etc/sonic/neighsyncd/neighsyncd.conf
sudo chown sonic:sonic /etc/sonic/neighsyncd/neighsyncd.conf
```

#### 5. Test manual execution

```bash
# Run as sonic user for debugging
sudo -u sonic /usr/local/bin/sonic-neighsyncd

# Check output for error messages
```

### Common Causes and Fixes

#### Redis Not Running

**Error:**
```
Error: Failed to connect to Redis at [::1]:6379: Connection refused
```

**Fix:**
```bash
# Start Redis
sudo systemctl start redis.service

# Enable on boot
sudo systemctl enable redis.service

# Verify Redis is listening
redis-cli -h ::1 PING
```

#### Certificate Files Missing

**Error:**
```
Error: Failed to load server certificate: No such file or directory
```

**Fix:**
```bash
# Check certificate files exist
ls -l /etc/sonic/metrics/server/server-cert.pem
ls -l /etc/sonic/metrics/server/server-key.pem
ls -l /etc/sonic/metrics/ca/ca-cert.pem

# Generate certificates if missing
cd /path/to/sonic-swss/crates/neighsyncd
sudo ./install.sh --enable-mtls
```

#### Port Already in Use

**Error:**
```
Error: Failed to bind metrics server: Address already in use (os error 98)
```

**Fix:**
```bash
# Check what's using port 9091
sudo ss -tlnp | grep 9091

# Kill conflicting process
sudo kill <PID>

# Or change metrics port in configuration
```

#### Insufficient Permissions

**Error:**
```
Error: Permission denied (os error 13)
```

**Fix:**
```bash
# Check systemd service capabilities
systemctl cat neighsyncd.service | grep Capability

# Verify sonic user exists
id sonic

# Create sonic user if missing
sudo useradd -r -s /bin/false -d /var/run/sonic -c "SONiC Daemon User" sonic
```

---

## Redis Connection Issues

### Symptom

Service starts but constantly logs Redis errors.

### Diagnostic Steps

#### 1. Test Redis connectivity

```bash
# IPv6 loopback
redis-cli -h ::1 -p 6379 PING

# IPv4 loopback (if configured)
redis-cli -h 127.0.0.1 -p 6379 PING

# Check Redis binding
redis-cli CONFIG GET bind
```

#### 2. Check Redis logs

```bash
sudo journalctl -u redis.service -f
```

#### 3. Verify Redis configuration

```bash
# Check Redis is listening on IPv6
sudo ss -tlnp | grep 6379

# Should show:
# [::1]:6379  (IPv6 loopback)
```

### Common Fixes

#### Redis Not Listening on IPv6

**Fix:**
```bash
# Edit Redis configuration
sudo vi /etc/redis/redis.conf

# Find and change:
bind 127.0.0.1
# To:
bind ::1 127.0.0.1

# Restart Redis
sudo systemctl restart redis.service
```

#### Redis Authentication Enabled

**Error:**
```
Error: NOAUTH Authentication required
```

**Fix:**
```bash
# Option 1: Disable Redis authentication (if not needed)
sudo vi /etc/redis/redis.conf
# Comment out: requirepass <password>

# Option 2: Configure password in neighsyncd.conf
[redis]
password = "your-redis-password"
```

#### Redis Database Corruption

**Error:**
```
Error: MISCONF Redis is configured to save RDB snapshots, but it is currently not able to persist on disk
```

**Fix:**
```bash
# Check disk space
df -h

# Fix Redis write permissions
sudo chown redis:redis /var/lib/redis

# Disable RDB persistence (if not needed)
redis-cli CONFIG SET stop-writes-on-bgsave-error no
```

---

## Netlink Socket Errors

### Symptom

Errors about netlink socket or missing neighbor events.

### Diagnostic Steps

#### 1. Check kernel support

```bash
# Verify netlink route support
grep CONFIG_NETLINK_ROUTE /boot/config-$(uname -r)
# Should output: CONFIG_NETLINK_ROUTE=y

# Check neighbor subsystem
ip -6 neigh show
ip -4 neigh show
```

#### 2. Check socket buffer size

```bash
# View current socket buffer limits
sysctl net.core.rmem_max
sysctl net.core.rmem_default

# Check for buffer overflows
journalctl -u neighsyncd.service | grep -i ENOBUFS
```

#### 3. Monitor netlink events

```bash
# Monitor neighbor events in real-time
ip -6 monitor neigh
```

### Common Fixes

#### Socket Buffer Overflow (ENOBUFS)

**Error:**
```
Error: Netlink socket buffer overflow (ENOBUFS)
```

**Fix:**
```bash
# Increase socket buffer size
sudo sysctl -w net.core.rmem_max=2097152  # 2 MB
sudo sysctl -w net.core.rmem_default=262144  # 256 KB

# Make permanent
echo "net.core.rmem_max = 2097152" | sudo tee -a /etc/sysctl.conf
echo "net.core.rmem_default = 262144" | sudo tee -a /etc/sysctl.conf

# Also increase neighsyncd socket buffer:
# Edit /etc/sonic/neighsyncd/neighsyncd.conf
[netlink]
socket_buffer_size = 1048576  # 1 MB

# Restart service
sudo systemctl restart neighsyncd.service
```

#### Permission Denied (CAP_NET_ADMIN)

**Error:**
```
Error: Permission denied opening netlink socket (os error 13)
```

**Fix:**
```bash
# Verify systemd service has CAP_NET_ADMIN
systemctl cat neighsyncd.service | grep Capability

# Should include:
# CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_RAW
# AmbientCapabilities=CAP_NET_ADMIN CAP_NET_RAW

# If missing, edit /etc/systemd/system/neighsyncd.service
sudo systemctl daemon-reload
sudo systemctl restart neighsyncd.service
```

---

## High Memory Usage

### Symptom

neighsyncd process using excessive memory (> 256 MB).

### Diagnostic Steps

#### 1. Check current memory usage

```bash
# Process memory
ps aux | grep sonic-neighsyncd

# Detailed memory breakdown
sudo pmap -x $(pgrep sonic-neighsyncd)

# Via metrics
curl -k --cert /etc/sonic/metrics/clients/prometheus/client-cert.pem \
     --key /etc/sonic/metrics/clients/prometheus/client-key.pem \
     https://[::1]:9091/metrics | grep memory_bytes
```

#### 2. Check neighbor table size

```bash
# Kernel neighbor count
ip -6 neigh show | wc -l
ip -4 neigh show | wc -l

# Redis neighbor count
redis-cli -h ::1 KEYS "NEIGH_TABLE:*" | wc -l

# Estimated memory per neighbor: ~300 bytes
```

#### 3. Check for memory leaks

```bash
# Monitor memory over time
watch -n 1 'ps aux | grep sonic-neighsyncd'

# Profile with valgrind (if available)
valgrind --leak-check=full /usr/local/bin/sonic-neighsyncd
```

### Common Fixes

#### Large Neighbor Table

**Fix:**
```bash
# This is expected for large deployments

# Estimated memory usage:
# 10,000 neighbors × 300 bytes = ~3 MB
# 100,000 neighbors × 300 bytes = ~30 MB
# Plus base overhead: ~50 MB

# If memory usage is problematic, tune:
[performance]
# Reduce batch buffer size
batch_size = 50

# Reduce event queue depth
queue_depth = 5000
```

#### Memory Leak (Batch Buffer Not Flushing)

**Error:**
Memory grows continuously without bound.

**Fix:**
```bash
# Check batch flush timeout
journalctl -u neighsyncd.service | grep "Flushing batch"

# Reduce batch timeout
[performance]
batch_timeout_ms = 50  # Flush more frequently

# Restart service
sudo systemctl restart neighsyncd.service
```

#### Enable Memory Limit

**Fix:**
```bash
# Edit systemd service
sudo systemctl edit neighsyncd.service

# Add:
[Service]
MemoryMax=256M
MemoryHigh=200M

# Reload and restart
sudo systemctl daemon-reload
sudo systemctl restart neighsyncd.service
```

---

## High CPU Usage

### Symptom

neighsyncd consuming high CPU (> 50%).

### Diagnostic Steps

#### 1. Check CPU usage

```bash
# Top processes
top -p $(pgrep sonic-neighsyncd)

# CPU breakdown
ps -p $(pgrep sonic-neighsyncd) -o %cpu,%mem,cmd
```

#### 2. Check event rate

```bash
# Monitor event processing rate
curl -k --cert /etc/sonic/metrics/clients/prometheus/client-cert.pem \
     --key /etc/sonic/metrics/clients/prometheus/client-key.pem \
     https://[::1]:9091/metrics | grep neighbors_processed_total

# Monitor netlink events
ip -6 monitor neigh | pv -l > /dev/null
```

#### 3. Profile CPU usage

```bash
# CPU profiling with perf
cd /path/to/sonic-swss/crates/neighsyncd
sudo ./profile.sh netlink_parsing 30

# Check profiling report
cat target/profiling/netlink_parsing.report.txt
```

### Common Fixes

#### High Event Rate

**Fix:**
```bash
# Increase batch size to reduce per-event overhead
[performance]
batch_size = 500  # Larger batches
worker_threads = 8  # More parallelism

# Restart service
sudo systemctl restart neighsyncd.service
```

#### Debug Logging Enabled

**Error:**
CPU high due to excessive logging.

**Fix:**
```bash
# Reduce log level
[logging]
level = "warn"  # Only warnings and errors

# Or via environment variable
export RUST_LOG="neighsyncd=warn"

# Restart service
sudo systemctl restart neighsyncd.service
```

#### FxHash Not Enabled

**Fix:**
```bash
# Rebuild with FxHash optimization
cd /path/to/sonic-workspace/sonic-swss
cargo build --release -p sonic-neighsyncd --features perf-fxhash

# Reinstall binary
sudo cp target/release/sonic-neighsyncd /usr/local/bin/
sudo systemctl restart neighsyncd.service
```

---

## Metrics Endpoint Issues

### Symptom

Cannot access metrics endpoint at `https://[::1]:9091/metrics`.

### Diagnostic Steps

#### 1. Check if metrics server is listening

```bash
sudo ss -tlnp | grep 9091

# Should show:
# [::1]:9091  LISTEN  ...  sonic-neighsyncd
```

#### 2. Test mTLS connection

```bash
# Test with OpenSSL
openssl s_client -connect [::1]:9091 \
  -CAfile /etc/sonic/metrics/ca/ca-cert.pem \
  -cert /etc/sonic/metrics/clients/prometheus/client-cert.pem \
  -key /etc/sonic/metrics/clients/prometheus/client-key.pem \
  -tls1_3

# Should output: Verify return code: 0 (ok)
```

#### 3. Test metrics endpoint

```bash
# With client certificates
curl -k --cert /etc/sonic/metrics/clients/prometheus/client-cert.pem \
     --key /etc/sonic/metrics/clients/prometheus/client-key.pem \
     https://[::1]:9091/metrics

# Health endpoint
curl -k --cert /etc/sonic/metrics/clients/prometheus/client-cert.pem \
     --key /etc/sonic/metrics/clients/prometheus/client-key.pem \
     https://[::1]:9091/health
```

### Common Fixes

#### Certificate Expired

**Error:**
```
Error: certificate has expired
```

**Fix:**
```bash
# Check certificate expiration
openssl x509 -in /etc/sonic/metrics/server/server-cert.pem -noout -dates

# Regenerate certificates
cd /path/to/sonic-swss/crates/neighsyncd
sudo ./install.sh --enable-mtls

# Restart service
sudo systemctl restart neighsyncd.service
```

#### Wrong Certificate Authority

**Error:**
```
Error: unable to verify the first certificate
```

**Fix:**
```bash
# Verify certificate chain
openssl verify -CAfile /etc/sonic/metrics/ca/ca-cert.pem \
  /etc/sonic/metrics/server/server-cert.pem

# Should output: OK

# If failed, regenerate with correct CA
```

#### Firewall Blocking Port

**Fix:**
```bash
# Check firewall rules
sudo ip6tables -L -n | grep 9091

# Allow metrics port (IPv6)
sudo ip6tables -A INPUT -p tcp --dport 9091 -s ::1 -j ACCEPT

# Or disable metrics mTLS for debugging (NOT production)
[metrics]
mtls_enabled = false
```

---

## Warm Restart Problems

### Symptom

Warm restart not caching or reconciling state correctly.

### Diagnostic Steps

#### 1. Check warm restart flag

```bash
# Check if warm restart is enabled
redis-cli -h ::1 GET "WARM_RESTART_ENABLE_TABLE|neighsyncd"

# Should return: "true" if enabled
```

#### 2. Check cached state

```bash
# Check if cache exists
redis-cli -h ::1 EXISTS "WARM_RESTART_NEIGHSYNCD_CACHE"

# View cached neighbors
redis-cli -h ::1 HGETALL "WARM_RESTART_NEIGHSYNCD_CACHE"
```

#### 3. Monitor warm restart logs

```bash
journalctl -u neighsyncd.service | grep -i "warm restart"

# Expected sequence:
# - "Warm restart mode detected"
# - "Loaded X cached neighbors from Redis"
# - "Starting reconciliation timer"
# - "Reconciliation complete: X added, Y updated, Z deleted"
```

### Common Fixes

#### Cache Not Saving

**Error:**
Warm restart flag set, but no cache created.

**Fix:**
```bash
# Manually trigger cache save
redis-cli -h ::1 SET "WARM_RESTART_ENABLE_TABLE|neighsyncd" "true"

# Restart service (will trigger cache)
sudo systemctl restart neighsyncd.service

# Check logs
journalctl -u neighsyncd.service -f
```

#### Reconciliation Timeout Too Short

**Error:**
Reconciliation completes before kernel state is stable.

**Fix:**
```bash
# Increase reconcile timeout
[performance]
reconcile_timeout_ms = 10000  # 10 seconds

# Restart service
sudo systemctl restart neighsyncd.service
```

#### State Mismatch After Reconciliation

**Error:**
Redis and kernel neighbor counts don't match after warm restart.

**Fix:**
```bash
# Force full reconciliation
redis-cli -h ::1 DEL "WARM_RESTART_NEIGHSYNCD_CACHE"
redis-cli -h ::1 DEL "WARM_RESTART_ENABLE_TABLE|neighsyncd"

# Cold restart (full resync)
sudo systemctl restart neighsyncd.service

# Verify counts match
KERNEL=$(ip -6 neigh show | wc -l)
REDIS=$(redis-cli -h ::1 KEYS "NEIGH_TABLE:*" | wc -l)
echo "Kernel: $KERNEL, Redis: $REDIS, Diff: $((KERNEL - REDIS))"
```

---

## Performance Issues

### Symptom

High latency or low throughput.

### Diagnostic Steps

#### 1. Check metrics

```bash
# Event processing latency (p99)
curl -k --cert /etc/sonic/metrics/clients/prometheus/client-cert.pem \
     --key /etc/sonic/metrics/clients/prometheus/client-key.pem \
     https://[::1]:9091/metrics | grep event_latency_seconds

# Batch size distribution
curl -k --cert /etc/sonic/metrics/clients/prometheus/client-cert.pem \
     --key /etc/sonic/metrics/clients/prometheus/client-key.pem \
     https://[::1]:9091/metrics | grep batch_size
```

#### 2. Run benchmarks

```bash
cd /path/to/sonic-workspace/sonic-swss
cargo bench -p sonic-neighsyncd

# Check results in target/criterion/
firefox target/criterion/report/index.html
```

#### 3. Profile with perf

```bash
cd /path/to/sonic-swss/crates/neighsyncd
sudo ./profile.sh event_processing 60

# View flamegraph
firefox target/profiling/event_processing.svg
```

### Common Fixes

#### Small Batch Size

**Fix:**
```bash
[performance]
batch_size = 100  # Increase from default
batch_timeout_ms = 100
```

#### Too Few Worker Threads

**Fix:**
```bash
[performance]
worker_threads = 8  # Match CPU cores
```

#### Redis Latency

**Fix:**
```bash
# Check Redis latency
redis-cli -h ::1 --latency

# Tune Redis
redis-cli CONFIG SET tcp-backlog 511
redis-cli CONFIG SET timeout 0
```

---

## Log Analysis

### View Logs

```bash
# Last 100 lines
sudo journalctl -u neighsyncd.service -n 100

# Follow in real-time
sudo journalctl -u neighsyncd.service -f

# Since specific time
sudo journalctl -u neighsyncd.service --since "2026-01-25 10:00:00"

# Filter by priority
sudo journalctl -u neighsyncd.service -p err  # Errors only
sudo journalctl -u neighsyncd.service -p warning  # Warnings and above
```

### Structured Log Queries

For JSON logs:

```bash
# Extract error messages
journalctl -u neighsyncd.service -o json | jq 'select(.PRIORITY=="3") | .MESSAGE'

# Count errors by type
journalctl -u neighsyncd.service -o json | jq -r '.MESSAGE' | grep -i error | sort | uniq -c

# Average event latency from logs
journalctl -u neighsyncd.service -o json | jq '.fields.latency_ms' | awk '{sum+=$1; count++} END {print sum/count}'
```

### Common Log Patterns

```bash
# Find neighbor add events
journalctl -u neighsyncd.service | grep "Neighbor added"

# Find Redis errors
journalctl -u neighsyncd.service | grep -i "redis error"

# Find netlink errors
journalctl -u neighsyncd.service | grep -i "netlink error"

# Find warm restart events
journalctl -u neighsyncd.service | grep -i "warm restart"
```

---

## Debug Mode

### Enable Debug Logging

```bash
# Method 1: Environment variable (temporary)
sudo systemctl edit neighsyncd.service

# Add:
[Service]
Environment="RUST_LOG=neighsyncd=debug"

# Reload and restart
sudo systemctl daemon-reload
sudo systemctl restart neighsyncd.service

# Method 2: Configuration file (permanent)
[logging]
level = "debug"
```

### Enable Trace Logging (Very Verbose)

```bash
# WARNING: Extremely verbose, use only for debugging specific issues

[Service]
Environment="RUST_LOG=neighsyncd=trace"

# Or more granular:
Environment="RUST_LOG=neighsyncd::neighsync=trace,neighsyncd=debug"
```

### Capture Stack Traces

```bash
# Enable backtraces
[Service]
Environment="RUST_BACKTRACE=1"  # Short backtraces
# Or:
Environment="RUST_BACKTRACE=full"  # Full backtraces
```

---

## Common Error Messages

### "Redis connection failed"

**Meaning**: Cannot connect to Redis server.

**Fix**: See [Redis Connection Issues](#redis-connection-issues)

### "Netlink socket buffer overflow"

**Meaning**: Too many neighbor events, socket buffer full.

**Fix**: Increase socket buffer size (see [Netlink Socket Errors](#netlink-socket-errors))

### "Certificate verification failed"

**Meaning**: mTLS client certificate invalid or expired.

**Fix**: Regenerate certificates (see [Metrics Endpoint Issues](#metrics-endpoint-issues))

### "Health status: Unhealthy (stall detected)"

**Meaning**: No neighbor events received for > 10 seconds.

**Fix**: Check kernel neighbor table activity with `ip -6 monitor neigh`

---

## Performance Profiling

See [profile.sh](../../../sonic-swss/crates/neighsyncd/profile.sh) script for detailed CPU profiling.

---

## Known Issues

1. **Netlink buffer overflow on high-density switches**: Increase socket_buffer_size to 1-2 MB
2. **Redis connection timeout during restart**: Increase redis.timeout_ms to 10000
3. **Certificate renewal requires restart**: Automatic certificate reload not implemented

---

## Getting Help

1. **GitHub Issues**: https://github.com/sonic-net/sonic-swss/issues
2. **SONiC Community**: https://groups.google.com/g/sonicproject
3. **Documentation**: `docs/rust/neighsyncd/`
4. **Logs**: Always include `journalctl -u neighsyncd.service -n 200` when reporting issues

---

**End of Troubleshooting Guide**
