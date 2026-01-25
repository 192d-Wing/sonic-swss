# neighsyncd Monitoring Runbooks

Operational procedures for monitoring and troubleshooting neighsyncd in production environments.

## Quick Diagnostics

### Check Service Status

```bash
# Service status
systemctl status sonic-neighsyncd

# Is process running?
pgrep -f sonic-neighsyncd

# Process resource usage
ps aux | grep sonic-neighsyncd

# Recent logs
journalctl -u sonic-neighsyncd -n 50 --no-pager
```

### Check Metrics Endpoint

```bash
# Verify metrics are exported
curl -s http://[::1]:9091/metrics | head -20

# Count total metric families
curl -s http://[::1]:9091/metrics | grep -E "^[a-z_]+_total|^[a-z_]+_bucket|^[a-z_]+_count" | wc -l

# Check specific metric
curl -s http://[::1]:9091/metrics | grep "neighsyncd_health_status"
```

### Check Dependencies

```bash
# Redis connectivity
redis-cli -h localhost -p 6379 ping

# Check Redis neighbors database
redis-cli -h localhost -p 6379 -n 0 HLEN "NEIGHBOR_TABLE:*"

# Netlink socket status
ss -ua | grep -i netlink

# Check CAP_NET_ADMIN capability
getcap /usr/local/bin/sonic-neighsyncd
```

---

## Alert Response Procedures

### Alert: NeighsyncHighErrorRate

**Severity**: Critical
**Threshold**: > 1% of events failing
**Typical Causes**: Redis issues, netlink buffer overflow, corrupted events

**Diagnosis**:

1. Check Redis connectivity:
   ```bash
   redis-cli PING
   ```

2. Check error types in logs:
   ```bash
   journalctl -u sonic-neighsyncd -n 200 | grep -i error | head -20
   ```

3. Check metrics detail:
   ```bash
   curl -s http://[::1]:9091/metrics | grep "neighsyncd_events_failed"
   curl -s http://[::1]:9091/metrics | grep "neighsyncd_redis_errors"
   curl -s http://[::1]:9091/metrics | grep "neighsyncd_netlink_errors"
   ```

**Resolution**:

- If Redis errors:
  - Check Redis service: `systemctl status redis`
  - Check Redis logs: `journalctl -u redis -n 50`
  - Restart Redis if needed: `systemctl restart redis`

- If netlink errors:
  - Check kernel logs: `dmesg | tail -20`
  - Verify netlink buffer size in config
  - Increase `socket_buffer_size` in neighsyncd.conf

- If persistent:
  - Restart neighsyncd: `systemctl restart sonic-neighsyncd`
  - Check for memory leaks: `ps aux | grep sonic-neighsyncd`

---

### Alert: NeighsyncRedisUnavailable

**Severity**: Critical
**Impact**: No neighbor updates synced to database
**Recovery Time**: Usually < 1 minute

**Diagnosis**:

1. Check Redis is running:
   ```bash
   systemctl is-active redis
   ```

2. Check Redis logs:
   ```bash
   journalctl -u redis -n 50
   ```

3. Check network connectivity:
   ```bash
   ping -6 ::1
   nc -zv ::1 6379
   ```

4. Check neighsyncd can connect:
   ```bash
   journalctl -u sonic-neighsyncd -n 50 | grep -i redis
   ```

**Resolution**:

- Start Redis if stopped:
  ```bash
  systemctl start redis
  ```

- If Redis fails to start:
  ```bash
  redis-server /etc/redis/redis.conf --loglevel debug
  ```

- Check Redis configuration:
  - Verify bind address includes `::1`
  - Check port 6379 is open
  - Verify no firewall rules blocking

- Restart neighsyncd to reconnect:
  ```bash
  systemctl restart sonic-neighsyncd
  ```

---

### Alert: NeighsyncNetlinkErrors

**Severity**: Critical
**Cause**: Kernel neighbor updates not being received
**Typical Issues**: Buffer overflow, missing CAP_NET_ADMIN, kernel incompatibility

**Diagnosis**:

1. Check kernel version:
   ```bash
   uname -r
   ```

2. Verify CAP_NET_ADMIN:
   ```bash
   getcap /usr/local/bin/sonic-neighsyncd
   # Should show: cap_net_admin,cap_net_raw=ep
   ```

3. Check netlink buffer size:
   ```bash
   sysctl net.ipv4.netlink_max_ack_backlog
   sysctl net.core.rmem_max
   ```

4. Check for dropped messages:
   ```bash
   journalctl -u sonic-neighsyncd -n 100 | grep -i "buffer\|overflow\|dropped"
   ```

**Resolution**:

- Reinstall with correct capabilities:
  ```bash
  sudo ./install.sh
  ```

- Increase netlink buffer size in config:
  ```toml
  [netlink]
  socket_buffer_size = 8388608  # 8MB instead of default
  ```

- Increase system limits:
  ```bash
  echo "net.ipv4.netlink_max_ack_backlog=2000" >> /etc/sysctl.conf
  sysctl -p
  ```

- Restart neighsyncd:
  ```bash
  systemctl restart sonic-neighsyncd
  ```

---

### Alert: NeighsyncHighMemoryUsage

**Severity**: Warning → Critical
**Threshold**: 150MB warning, 200MB critical
**Systemd Limit**: 256MB (MemoryLimit in service file)

**Diagnosis**:

1. Check memory usage:
   ```bash
   ps aux | grep sonic-neighsyncd | grep -v grep
   # Check RSS column (resident memory)
   ```

2. Check neighbor count:
   ```bash
   redis-cli HGETALL "NEIGHBOR_TABLE:Ethernet0" | wc -l
   ```

3. Get detailed memory info:
   ```bash
   cat /proc/$(pgrep -f sonic-neighsyncd)/smaps | tail -20
   ```

**Resolution**:

- Check for memory leaks with sustained load:
  ```bash
  watch -n 5 'ps aux | grep sonic-neighsyncd | grep -v grep'
  ```

- If memory increases continuously:
  - Restart neighsyncd: `systemctl restart sonic-neighsyncd`
  - Report as potential memory leak

- If memory is stable but high:
  - Increase MemoryLimit in systemd service
  - Monitor neighbor count growth
  - Consider neighbor table pruning

---

### Alert: NeighsyncHighLatency

**Severity**: Warning
**Threshold**: p99 latency > 100ms
**Typical Causes**: Redis latency, high neighbor churn, system load

**Diagnosis**:

1. Check processing rate:
   ```bash
   curl -s http://[::1]:9091/metrics | grep "neighsyncd_neighbors_processed_total"
   ```

2. Check queue depth:
   ```bash
   curl -s http://[::1]:9091/metrics | grep "neighsyncd_queue_depth"
   ```

3. Check batch sizes:
   ```bash
   curl -s http://[::1]:9091/metrics | grep "neighsyncd_batch_size"
   ```

4. Check Redis latency:
   ```bash
   redis-cli --latency
   redis-cli --latency-history
   ```

5. Check system load:
   ```bash
   uptime
   top -b -n 1 | head -20
   ```

**Resolution**:

- If Redis latency is high:
  - Check Redis CPU: `top` and find redis-server
  - Check Redis slow log: `redis-cli SLOWLOG GET 10`
  - Increase batch size for efficiency: `batch_size = 500` in config

- If system load is high:
  - Check other processes: `top`
  - Check disk I/O: `iostat -x 1`
  - Consider distributed deployment

- Increase batch timeout to improve throughput:
  ```toml
  [performance]
  batch_timeout_ms = 50  # Process batches more frequently
  ```

- Restart with adjusted config:
  ```bash
  systemctl restart sonic-neighsyncd
  ```

---

### Alert: NeighsyncUnhealthy

**Severity**: Critical
**Status**: 0 = Unhealthy, 0.5 = Degraded, 1.0 = Healthy

**Diagnosis**:

1. Check health metric:
   ```bash
   curl -s http://[::1]:9091/metrics | grep "neighsyncd_health_status"
   ```

2. Check all status indicators:
   ```bash
   curl -s http://[::1]:9091/metrics | grep -E "connected|health|stall"
   ```

3. Check dependencies:
   ```bash
   # Redis status
   curl -s http://[::1]:9091/metrics | grep "neighsyncd_redis_connected"

   # Netlink status
   curl -s http://[::1]:9091/metrics | grep "neighsyncd_netlink_connected"
   ```

**Resolution**:

- Run full diagnostics script:
  ```bash
  bash <(curl -s https://repo/neighsyncd-health-check.sh)
  ```

- Check most recent alerts:
  ```bash
  journalctl -u sonic-neighsyncd -n 50
  ```

- Restart daemon:
  ```bash
  systemctl restart sonic-neighsyncd
  ```

- If restarts don't help:
  - Check filesystem: `df -h`
  - Check system resources: `free -h`
  - Check logs for clues: `journalctl -u sonic-neighsyncd --no-pager | tail -100`

---

### Alert: NeighsyncProcessingStall

**Severity**: Warning
**Condition**: No events processed for 30+ seconds
**Timeout**: 60 seconds = unhealthy

**Diagnosis**:

1. Check time since last event:
   ```bash
   date
   curl -s http://[::1]:9091/metrics | grep "neighsyncd_last_event_timestamp"
   ```

2. Check if neighbors are being added to netlink:
   ```bash
   # Watch for netlink events
   ip monitor neigh
   ```

3. Check netlink socket status:
   ```bash
   ss -ua | grep -E "netlink|NETLINK"
   ```

4. Check if daemon is responsive:
   ```bash
   systemctl is-active sonic-neighsyncd
   curl -s http://[::1]:9091/metrics > /dev/null && echo "Responsive"
   ```

**Resolution**:

- If netlink is stuck:
  - Restart neighsyncd: `systemctl restart sonic-neighsyncd`
  - Check kernel logs: `dmesg | tail -20`

- If no neighbors being added:
  - Check if neighbor table has entries: `ip neigh show`
  - Monitor netlink events: `ip monitor neigh &`
  - Generate test events: Add test IP to interface

- If daemon not responsive:
  - Kill and restart: `pkill -9 sonic-neighsyncd && systemctl start sonic-neighsyncd`
  - Check system resources

---

### Alert: NeighsyncHighQueueDepth

**Severity**: Warning
**Threshold**: > 1000 pending events
**Indicates**: Processing can't keep up with input

**Diagnosis**:

1. Check queue depth and processing rate:
   ```bash
   curl -s http://[::1]:9091/metrics | grep -E "queue_depth|processing_rate"
   ```

2. Check batch size in use:
   ```bash
   curl -s http://[::1]:9091/metrics | grep "neighsyncd_batch_size"
   ```

3. Check Redis latency (likely bottleneck):
   ```bash
   redis-cli --latency
   ```

4. Check system resources:
   ```bash
   top -b -n 1 | head -20
   ```

**Resolution**:

- Increase batch size for efficiency:
  ```toml
  [performance]
  batch_size = 1000  # Process more neighbors per batch
  ```

- Reduce batch timeout to process faster:
  ```toml
  batch_timeout_ms = 50
  ```

- Increase worker threads:
  ```toml
  worker_threads = 8
  ```

- Check Redis performance:
  ```bash
  redis-cli INFO stats
  redis-cli SLOWLOG GET
  ```

- Restart with new config:
  ```bash
  systemctl restart sonic-neighsyncd
  ```

---

### Alert: NeighsyncCertificateExpiration

**Severity**: Warning
**Threshold**: 30 days before expiration
**Impact**: mTLS endpoint will stop working when expired

**Diagnosis**:

1. Check certificate expiration:
   ```bash
   openssl x509 -in /etc/sonic/neighsyncd/server.crt -noout -dates
   ```

2. Check if using CNSA 2.0 (P-384):
   ```bash
   openssl x509 -in /etc/sonic/neighsyncd/server.crt -text -noout | grep -A2 "Public Key"
   ```

**Resolution**:

1. Generate new certificates (see DEPLOYMENT.md):
   ```bash
   cd /etc/sonic/neighsyncd

   # Generate new server certificate (valid 2 years)
   openssl req -new -x509 -days 730 \
     -newkey ec:<(openssl ecparam -name secp384r1) \
     -sha384 -nodes \
     -keyout server.key -out server.crt \
     -subj "/CN=neighsyncd/O=SONiC"

   # Fix ownership
   chown sonic:sonic server.crt server.key
   chmod 600 server.key
   ```

2. Restart neighsyncd:
   ```bash
   systemctl restart sonic-neighsyncd
   ```

3. Verify new certificate:
   ```bash
   openssl x509 -in /etc/sonic/neighsyncd/server.crt -noout -dates
   ```

---

## Grafana Dashboard Usage

### Interpreting Dashboard Panels

**Neighbor Throughput**
- Shows rate of neighbor processing: processed, added, deleted
- Baseline: Depends on network churn (typically 10-100/sec)
- Alert: Should be > 1/sec on active network

**Error Rates**
- Failed events, netlink errors, Redis errors
- Baseline: Should be near zero
- Alert: Any persistent errors warrant investigation

**Event Latency (Percentiles)**
- p50, p95, p99 latency
- Baseline: p99 < 50ms
- Alert: p99 > 100ms indicates processing issues

**Memory Usage**
- RSS memory of neighsyncd process
- Baseline: 50-100MB depending on neighbor count
- Alert: > 200MB (systemd hard limit)

**Health Status**
- Color-coded: Green=Healthy, Yellow=Degraded, Red=Unhealthy
- Updates every 30 seconds
- Trends indicate onset of issues

**Connection Indicators**
- Redis: Green = Connected, Red = Disconnected
- Netlink: Green = Active, Red = Disconnected
- Both should always be green

**Redis Latency**
- p50, p95, p99 for Redis operations
- Baseline: p99 < 10ms
- Alert: > 50ms indicates Redis issues

**Queue Depth**
- Pending neighbors and event queue depth
- Baseline: Should be < 100
- Alert: > 1000 indicates processing lag

---

## Performance Tuning

### Optimize for Throughput

```toml
[performance]
batch_size = 1000              # Larger batches
batch_timeout_ms = 100         # Longer timeout for batches
worker_threads = 8             # More workers for parallel processing
queue_max_depth = 10000        # Allow larger queue

[netlink]
socket_buffer_size = 16777216  # 16MB socket buffer
timeout_ms = 5000              # Longer timeout
```

### Optimize for Low Latency

```toml
[performance]
batch_size = 100               # Smaller batches for faster processing
batch_timeout_ms = 10          # Process quickly
worker_threads = 4
queue_max_depth = 1000         # Small queue

[netlink]
socket_buffer_size = 2097152   # 2MB socket buffer
timeout_ms = 1000              # Short timeout
```

### Monitor Changes

```bash
# Baseline before changes
curl -s http://[::1]:9091/metrics > baseline.txt

# Make config changes
vi /etc/sonic/neighsyncd/neighsyncd.conf
systemctl restart sonic-neighsyncd

# Wait 5 minutes for stable state
sleep 300

# Compare metrics
curl -s http://[::1]:9091/metrics > after.txt
diff baseline.txt after.txt
```

---

## Maintenance

### Regular Health Checks

Daily:
```bash
# Check for alerts
journalctl -u sonic-neighsyncd -n 20 | grep -i alert

# Check error rates
curl -s http://[::1]:9091/metrics | grep "_errors_total"
```

Weekly:
```bash
# Review performance trends
# Check Grafana dashboard for anomalies

# Verify metrics export
curl -s http://[::1]:9091/metrics | wc -l
```

Monthly:
```bash
# Run full health diagnostics
./install.sh --health-check

# Review and rotate logs
journalctl --vacuum=30d

# Check for dependency updates
cargo outdated -p sonic-neighsyncd
```

### Log Rotation

Configure in `/etc/logrotate.d/sonic-neighsyncd`:

```
/var/log/sonic/neighsyncd.log {
    size 100M
    rotate 10
    compress
    delaycompress
    notifempty
    create 0640 sonic sonic
    postrotate
        systemctl reload sonic-neighsyncd > /dev/null 2>&1 || true
    endscript
}
```

---

## Emergency Procedures

### Circuit Breaker: Force Stop

If service is consuming resources:

```bash
systemctl stop sonic-neighsyncd
# Wait 10 seconds
ps aux | grep sonic-neighsyncd  # Verify stopped
```

### Circuit Breaker: Force Restart

```bash
systemctl restart sonic-neighsyncd
sleep 5
systemctl status sonic-neighsyncd
```

### Hard Reset

If restart doesn't help:

```bash
# Stop service
systemctl stop sonic-neighsyncd

# Clear caches
redis-cli FLUSHDB

# Restart
systemctl start sonic-neighsyncd

# Monitor startup
journalctl -u sonic-neighsyncd -f
```

### Rollback to Previous Version

```bash
# Restore previous binary
cp /usr/local/bin/sonic-neighsyncd.backup /usr/local/bin/sonic-neighsyncd

# Restart
systemctl restart sonic-neighsyncd
```

---

## Contact & Escalation

- **On-Call**: Check PagerDuty
- **Slack**: #sonic-neighbors channel
- **Issues**: https://github.com/sonic-net/sonic-swss/issues
- **Documentation**: See DEPLOYMENT.md, CONFIGURATION.md

---

## Changelog

- v1.0.0 - Initial runbook creation
- v1.1.0 - Added certificate expiration procedures
- v1.2.0 - Added performance tuning section
