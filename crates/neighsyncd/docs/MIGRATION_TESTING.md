# neighsyncd Migration Testing Procedures

Comprehensive testing guide for validating the C++ to Rust migration in production environments.

## Test Phases

### Phase 1: Pre-Deployment Validation (Local/Lab)

#### 1.1: Build Verification
```bash
# Prerequisites
apt-get install -y cargo rustc libc-dev libssl-dev pkg-config

# Build for current architecture
cd crates/neighsyncd
cargo build --release

# Verify binary exists
file target/release/neighsyncd
# Should output: ELF 64-bit LSB executable

# Check dependencies
ldd target/release/neighsyncd | head -10
# Verify no missing libraries

# Get binary size
ls -lh target/release/neighsyncd
# Typical: 10-15MB
```

#### 1.2: Unit Test Execution
```bash
# Run all tests
cargo test --lib

# Expected output
# test result: ok. 39 passed; 0 failed

# Run specific test suites
cargo test --lib advanced_health --verbose
cargo test --lib metrics --verbose
cargo test --lib netlink --verbose

# Generate test coverage (optional)
cargo tarpaulin --out Html
```

#### 1.3: Integration Test Execution
```bash
# Requires Docker for testcontainers
which docker || apt-get install -y docker.io

# Start docker
systemctl start docker

# Run Redis integration tests
cargo test --test redis_integration_tests -- --ignored --nocapture

# Run warm restart tests
cargo test --test warm_restart_integration -- --ignored --nocapture

# Expected output
# test result: ok. 34 passed; 0 failed
```

#### 1.4: Code Quality Checks
```bash
# Run clippy (Rust linter)
cargo clippy --all

# Expected: "Finished with no warnings"

# Run fmt check
cargo fmt --check

# Run audit
cargo audit

# All should pass
```

#### 1.5: Performance Baseline
```bash
# Run benchmarks
cargo bench -p sonic-neighsyncd 2>&1 | tee benchmark_baseline.txt

# Extract summary
grep -E "Benchmark|result" benchmark_baseline.txt

# Save for comparison
cp benchmark_baseline.txt ~/baseline.txt
```

---

### Phase 2: Staging Environment Testing (Optional but Recommended)

#### 2.1: Deployment to Staging

```bash
# Create test environment
vagrant up  # or use actual staging hardware

# Copy binary
scp crates/neighsyncd/target/release/neighsyncd staging-host:/tmp/

# SSH to staging host
ssh staging-host

# Backup current binary
sudo cp /usr/local/bin/sonic-neighsyncd /usr/local/bin/sonic-neighsyncd.backup

# Install new binary
sudo cp /tmp/neighsyncd /usr/local/bin/sonic-neighsyncd
sudo chown root:root /usr/local/bin/sonic-neighsyncd
sudo chmod 755 /usr/local/bin/sonic-neighsyncd

# Install configuration (if needed)
sudo mkdir -p /etc/sonic/neighsyncd
sudo cp crates/neighsyncd/neighsyncd.conf.example /etc/sonic/neighsyncd/neighsyncd.conf
```

#### 2.2: Basic Functionality Tests

```bash
# Start service
sudo systemctl start sonic-neighsyncd
sleep 2

# Verify running
ps aux | grep sonic-neighsyncd | grep -v grep

# Check logs
sudo journalctl -u sonic-neighsyncd -n 20

# Verify no errors
sudo journalctl -u sonic-neighsyncd | grep -i error | head -5
```

#### 2.3: Neighbor Learning Test

```bash
# Clear existing neighbors
redis-cli EVAL "return redis.call('del', unpack(redis.call('keys', 'NEIGHBOR_TABLE:*')))" 0

# Wait a moment
sleep 2

# Add test neighbor
ip -6 neigh add 2001:db8::test dev Ethernet0 lladdr 00:11:22:33:44:55

# Verify in Redis
redis-cli HGETALL "NEIGHBOR_TABLE:Ethernet0" | grep 2001:db8::test

# Expected output:
# 2001:db8::test
# <neighbor_value>
```

#### 2.4: Metrics Verification

```bash
# Check metrics endpoint
curl -s http://[::1]:9091/metrics | head -20

# Verify key metrics exist
curl -s http://[::1]:9091/metrics | grep -E "neighsyncd_neighbors_processed|neighsyncd_health_status|neighsyncd_redis_connected"

# Should output lines like:
# neighsyncd_neighbors_processed_total 5
# neighsyncd_health_status 1.0
# neighsyncd_redis_connected 1
```

#### 2.5: Load Testing

```bash
# Create 1000 test neighbors
for i in {1..1000}; do
    ip -6 neigh add 2001:db8::$i dev Ethernet0 lladdr 00:11:22:33:$(printf '%02x:%02x' $((i/256)) $((i%256)))
done

# Monitor metrics during load
watch -n 1 'curl -s http://[::1]:9091/metrics | grep -E "processed|queue_depth"'

# Wait for processing to complete
sleep 10

# Verify count
redis-cli HLEN "NEIGHBOR_TABLE:Ethernet0"
# Should be approximately 1000

# Check memory
ps aux | grep sonic-neighsyncd | awk '{print $6 " KB"}'
# Should be < 100MB
```

#### 2.6: Error Handling Test

```bash
# Test Redis failure recovery
redis-cli SHUTDOWN

# Monitor daemon response
journalctl -u sonic-neighsyncd -f &

# Wait 5 seconds
sleep 5

# Restart Redis
redis-server -d

# Verify reconnection
curl -s http://[::1]:9091/metrics | grep "neighsyncd_redis_connected"
# Should see: neighsyncd_redis_connected 1
```

#### 2.7: Warm Restart Test

```bash
# Stop daemon
sudo systemctl stop sonic-neighsyncd

# Verify warm restart cache created
redis-cli HLEN "WARM_RESTART_NEIGHSYNCD_TABLE"
# Should be >= 0 (depends on earlier operations)

# Restart daemon
sudo systemctl start sonic-neighsyncd

# Verify restoration
redis-cli HLEN "NEIGHBOR_TABLE:Ethernet0"
# Should match pre-stop count

# Check no duplicates created
journalctl -u sonic-neighsyncd -n 10 | grep -i reconcil
```

#### 2.8: Staging Validation Checklist

- [ ] Service starts cleanly
- [ ] No errors in logs
- [ ] Metrics endpoint responds
- [ ] Health status = 1.0 (healthy)
- [ ] Redis connected = 1
- [ ] Netlink connected = 1
- [ ] Neighbor learning works
- [ ] 1000 neighbors processed in < 2 seconds
- [ ] Memory < 100MB
- [ ] Redis failure recovery works
- [ ] Warm restart works
- [ ] No performance regressions

---

### Phase 3: Production Deployment

#### 3.1: Pre-Deployment Checklist

- [ ] All staging tests passed
- [ ] Baseline metrics documented
- [ ] Runbook reviewed by ops team
- [ ] Rollback procedure tested
- [ ] Monitoring alerts configured
- [ ] Dashboard created in Grafana
- [ ] On-call team notified
- [ ] Maintenance window scheduled
- [ ] Backup of C++ binary created
- [ ] Configuration backup created

#### 3.2: Deployment Procedure

```bash
# Scheduled maintenance window (off-peak)
# 1. Notify network operations center
# 2. Create ticket for change management

# On target device:

# Step 1: Verify current state
journalctl -u sonic-neighsyncd -n 5
redis-cli HLEN "NEIGHBOR_TABLE:*"
ps aux | grep sonic-neighsyncd | awk '{print $6}'

# Step 2: Backup
cp /usr/local/bin/sonic-neighsyncd /usr/local/bin/sonic-neighsyncd.backup.$(date +%s)
mkdir -p /etc/sonic/neighsyncd.backup
cp -r /etc/sonic/neighsyncd/* /etc/sonic/neighsyncd.backup/ 2>/dev/null || true

# Step 3: Install
cp /tmp/neighsyncd /usr/local/bin/sonic-neighsyncd
chmod 755 /usr/local/bin/sonic-neighsyncd

# Step 4: Start
systemctl stop sonic-neighsyncd
systemctl start sonic-neighsyncd

# Step 5: Wait for stability
sleep 5

# Step 6: Verify
systemctl is-active sonic-neighsyncd
systemctl status sonic-neighsyncd

# Step 7: Monitor
journalctl -u sonic-neighsyncd -f &
watch -n 1 'curl -s http://[::1]:9091/metrics | grep health_status'

# Step 8: Allow time to stabilize (2-5 minutes)
sleep 120
```

#### 3.3: Production Validation (Post-Deployment)

```bash
# 1. Service Health
systemctl is-active sonic-neighsyncd
systemctl status sonic-neighsyncd

# 2. Metrics Collection
curl -s http://[::1]:9091/metrics > /tmp/metrics-after.txt
cat /tmp/metrics-after.txt | grep -E "up|health|connected"

# 3. Neighbor Count
redis-cli HLEN "NEIGHBOR_TABLE:Ethernet0" > /tmp/neighbor-count-after.txt

# 4. Performance Check
redis-cli INFO stats | grep "total_net_input_bytes"

# 5. Error Rates
curl -s http://[::1]:9091/metrics | grep "errors_total"

# 6. Health Status
curl -s http://[::1]:9091/metrics | grep "neighsyncd_health_status"
# Should be 1.0 (Healthy)

# 7. Memory Usage
ps aux | grep sonic-neighsyncd | awk '{print "Memory: " $6 " KB"}'
# Should be < 100MB

# 8. CPU Usage
top -b -n 1 -p $(pgrep -f sonic-neighsyncd) | tail -1 | awk '{print "CPU: " $9 "%"}'
```

#### 3.4: Continuous Monitoring (Next 24 Hours)

```bash
# Set up continuous monitoring
watch -n 30 'echo "=== Health ===" && curl -s http://[::1]:9091/metrics | grep health_status && \
              echo "=== Neighbors ===" && redis-cli HLEN "NEIGHBOR_TABLE:Ethernet0" && \
              echo "=== Errors ===" && curl -s http://[::1]:9091/metrics | grep errors_total'

# Set up log monitoring
journalctl -u sonic-neighsyncd -f | grep -v "debug" | tee /tmp/neighsyncd-prod.log &

# Monitor for alerts
watch -n 5 'curl -s http://localhost:9090/api/v1/alerts | jq .data.alerts | grep neighsyncd'
```

#### 3.5: Production Validation Checklist

- [ ] Service running without restarts
- [ ] Health status = Healthy (1.0)
- [ ] Redis connected = 1
- [ ] Netlink connected = 1
- [ ] No error rate increase
- [ ] No memory leak (stable RSS)
- [ ] Neighbor count stable
- [ ] No increase in latency
- [ ] Metrics exported correctly
- [ ] No alerts triggered
- [ ] Logs show normal operation
- [ ] Peer devices unaffected
- [ ] BGP sessions stable
- [ ] Network performance normal

---

### Phase 4: Extended Monitoring (72 Hours)

```bash
# Daily checks
# Day 1 (after 24 hours)
date
curl -s http://[::1]:9091/metrics | grep -E "processed_total|health_status"
redis-cli HLEN "NEIGHBOR_TABLE:Ethernet0"
ps aux | grep sonic-neighsyncd | awk '{print "Memory: " $6 " KB"}'

# Day 2
# Repeat day 1 checks

# Day 3
# Repeat day 1 checks
# Compare with baseline
```

---

## Rollback Procedures

### Quick Rollback (< 1 minute)

```bash
# Only if daemon is misbehaving

# Step 1: Stop new version
systemctl stop sonic-neighsyncd

# Step 2: Restore backup
cp /usr/local/bin/sonic-neighsyncd.backup /usr/local/bin/sonic-neighsyncd

# Step 3: Start old version
systemctl start sonic-neighsyncd

# Step 4: Verify
sleep 3
systemctl is-active sonic-neighsyncd
journalctl -u sonic-neighsyncd -n 10
```

### Complete Rollback with Cache Clear

```bash
# If warm restart cache is suspected corrupt

systemctl stop sonic-neighsyncd

# Clear cache
redis-cli DEL "WARM_RESTART_NEIGHSYNCD_TABLE"

# Clear neighbor table (will relearn)
redis-cli EVAL "return redis.call('del', unpack(redis.call('keys', 'NEIGHBOR_TABLE:*')))" 0

# Restore C++ binary
cp /usr/local/bin/sonic-neighsyncd.backup.* /usr/local/bin/sonic-neighsyncd

# Start
systemctl start sonic-neighsyncd

# Monitor relearning (5-30 minutes)
watch -n 5 'redis-cli HLEN "NEIGHBOR_TABLE:Ethernet0"'
```

---

## Performance Testing

### Throughput Comparison

```bash
# Baseline with C++ version (before migration)
time {
    for i in {1..5000}; do
        ip -6 neigh add 2001:db8::$i dev Ethernet0 lladdr 00:11:22:33:$(printf '%02x:%02x' $((i/256)) $((i%256)))
    done
}

# Record time taken
# Expected: 30-60 seconds for 5000 additions

# Clear
redis-cli EVAL "return redis.call('del', unpack(redis.call('keys', 'NEIGHBOR_TABLE:*')))" 0

# After Rust migration
time {
    for i in {1..5000}; do
        ip -6 neigh add 2001:db8::$i dev Ethernet0 lladdr 00:11:22:33:$(printf '%02x:%02x' $((i/256)) $((i%256)))
    done
}

# Expected: 15-30 seconds (2x faster)
```

### Latency Testing

```bash
# Monitor P99 latency
while true; do
    curl -s http://[::1]:9091/metrics | \
    grep "neighsyncd_event_latency_seconds_bucket" | \
    tail -5
    sleep 10
done

# Expected P99: 40-60ms
```

### Resource Usage Testing

```bash
# Monitor memory over 24 hours
watch -n 300 'date >> /tmp/memory.log && \
              ps aux | grep sonic-neighsyncd | awk "{print \$6}" >> /tmp/memory.log'

# Analyze stability
cat /tmp/memory.log | tail -100 | sort -n | uniq -c | tail -20

# Expected: Stable within 5-10MB variance
```

---

## Automated Testing

### Continuous Integration Pipeline

```yaml
# .github/workflows/neighsyncd-test.yml
name: neighsyncd-test

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Unit Tests
        run: cargo test --lib -p sonic-neighsyncd

      - name: Clippy
        run: cargo clippy --lib -p sonic-neighsyncd

      - name: Format Check
        run: cargo fmt -p sonic-neighsyncd --check

      - name: Audit
        run: cargo audit -p sonic-neighsyncd

      - name: Build Release
        run: cargo build --release -p sonic-neighsyncd

      - name: Archive Binary
        uses: actions/upload-artifact@v2
        with:
          name: sonic-neighsyncd-${{ github.sha }}
          path: target/release/neighsyncd
```

---

## Test Reports

### Template: Post-Deployment Report

```markdown
# neighsyncd Migration - Post-Deployment Report

## Deployment Details
- **Date**: YYYY-MM-DD
- **Time**: HH:MM UTC
- **Duration**: X minutes
- **Deployed to**: Device/Environment

## Pre-Deployment Checklist
- [ ] Build verified
- [ ] Unit tests passed (39/39)
- [ ] Integration tests passed
- [ ] Staging validation passed
- [ ] Monitoring configured
- [ ] Rollback procedure tested

## Deployment Results
- **Status**: ✅ Success
- **Rollbacks**: 0
- **Issues**: None observed

## Performance Baseline
- Health Status: 1.0 (Healthy)
- Redis Connected: 1
- Netlink Connected: 1
- Memory Usage: XX MB
- Error Rate: 0%

## Validation Results
- Neighbor Learning: ✅ Pass
- Warm Restart: ✅ Pass
- Metrics Export: ✅ Pass
- Health Checks: ✅ Pass
- Load Test: ✅ Pass

## Monitoring (First 24 Hours)
- Average CPU: XX%
- Average Memory: XX MB
- Peak Memory: XX MB
- Error Events: 0
- Alerts Triggered: 0

## Sign-off
- Tested by: [Engineer Name]
- Approved by: [Manager Name]
- Date: YYYY-MM-DD
- Notes: [Any observations]
```

---

## Troubleshooting During Migration

### Common Issues and Solutions

#### Issue: Service Won't Start
```bash
# Check binary
/usr/local/bin/sonic-neighsyncd --version

# Check logs
journalctl -u sonic-neighsyncd -n 50

# Check dependencies
ldd /usr/local/bin/sonic-neighsyncd

# Rollback if needed
systemctl stop sonic-neighsyncd
cp /usr/local/bin/sonic-neighsyncd.backup /usr/local/bin/sonic-neighsyncd
systemctl start sonic-neighsyncd
```

#### Issue: Metrics Not Available
```bash
# Check service running
systemctl is-active sonic-neighsyncd

# Check port listening
netstat -tlnp | grep 9091

# Check if endpoint responding
curl -v http://[::1]:9091/metrics

# Restart metrics server
systemctl restart sonic-neighsyncd
```

#### Issue: Memory Leak Suspected
```bash
# Monitor memory
ps aux | grep sonic-neighsyncd | awk '{print $6}'

# Check for increasing value over 1 hour
for i in {1..60}; do
    ps aux | grep sonic-neighsyncd | awk "{print $(date +%s) \",\" \$6}" >> /tmp/memory.log
    sleep 60
done

# Analyze trend
awk -F, '{print $2}' /tmp/memory.log | tail -30 | sort -n | uniq -c

# If increasing: rollback
```

#### Issue: High Latency
```bash
# Check batch settings
grep -A 5 "\[performance\]" /etc/sonic/neighsyncd/neighsyncd.conf

# Reduce batch timeout
sed -i 's/batch_timeout_ms = .*/batch_timeout_ms = 10/' /etc/sonic/neighsyncd/neighsyncd.conf

# Restart
systemctl restart sonic-neighsyncd

# Monitor latency
curl -s http://[::1]:9091/metrics | grep event_latency
```

---

**Version**: 1.0.0
**Last Updated**: 2024-01-25
**Status**: Ready for Production Use
