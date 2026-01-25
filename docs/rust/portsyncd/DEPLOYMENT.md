# portsyncd Deployment Guide

Complete guide for deploying and operating portsyncd in production SONiC
switches.

## Pre-Deployment Checklist

- [ ] **Build**: `cargo build --release` succeeds with 0 warnings
- [ ] **Test**: `cargo test --all-features` passes 100% of tests
- [ ] **Performance**: Load tests show <10ms latency, >1000 eps throughput
- [ ] **Redis**: Redis 6.0+ running on localhost:6379
- [ ] **Linux**: Kernel 4.15+ with netlink socket support
- [ ] **Systemd**: systemd 230+ (for notify service type)
- [ ] **Space**: 50MB+ free disk space for binary and logs

## Building for Production

### Compile Optimized Binary

```bash
cd crates/portsyncd

# Release build with optimizations
cargo build --release

# Binary location
ls -lh target/release/portsyncd
```

**Build Flags**:

```bash
# With all features
cargo build --release --all-features

# Optimized for size
cargo build --release -Copt-level=z -Clto

# Optimized for speed
cargo build --release -Copt-level=3 -Cllvm-args=-vectorize-loops
```

### Verify Build

```bash
# Check binary
file target/release/portsyncd
# Output: ELF 64-bit LSB executable, x86-64, ...

# Run binary
./target/release/portsyncd --version

# Size
du -h target/release/portsyncd
# Typical: 5-10MB
```

## Installation

### Step 1: Copy Binary

```bash
sudo cp target/release/portsyncd /usr/local/bin/
sudo chmod 755 /usr/local/bin/portsyncd

# Verify
ls -l /usr/local/bin/portsyncd
```

### Step 2: Install Systemd Service

```bash
sudo cp portsyncd.service /etc/systemd/system/
sudo chmod 644 /etc/systemd/system/portsyncd.service

# Reload systemd
sudo systemctl daemon-reload
```

### Step 3: Create Configuration (Optional)

```bash
# Create config directory
sudo mkdir -p /etc/sonic

# Copy default config (if customized)
sudo cp portsyncd.conf /etc/sonic/portsyncd.conf
sudo chmod 644 /etc/sonic/portsyncd.conf

# Or use defaults (no config file needed)
```

### Step 4: Create Log Directory

```bash
sudo mkdir -p /var/log/portsyncd
sudo chmod 755 /var/log/portsyncd

# Or use systemd journal (recommended)
journalctl -u portsyncd
```

### Step 5: Verify Installation

```bash
# Check binary
which portsyncd
portsyncd --help

# Check systemd service
systemctl list-unit-files | grep portsyncd
systemctl status portsyncd

# Check configuration
cat /etc/sonic/portsyncd.conf
```

## Starting the Daemon

### Using Systemd (Recommended)

```bash
# Start daemon
sudo systemctl start portsyncd

# Check status
sudo systemctl status portsyncd

# Enable on boot
sudo systemctl enable portsyncd

# View logs
journalctl -u portsyncd -f

# Stop daemon
sudo systemctl stop portsyncd
```

### Manual Execution

```bash
# Direct execution (for debugging)
portsyncd

# With logging
RUST_LOG=info portsyncd

# Run in background
nohup portsyncd > /var/log/portsyncd.log 2>&1 &
```

## Configuration

### Default Configuration

If `/etc/sonic/portsyncd.conf` is missing, portsyncd uses built-in defaults:

```toml
[database]
redis_host = "127.0.0.1"
redis_port = 6379
config_db_number = 4
state_db_number = 6
connection_timeout_secs = 5
retry_interval_secs = 2

[performance]
max_event_queue = 1000
batch_timeout_ms = 100
max_latency_us = 10000
min_success_rate = 99.0

[health]
max_stall_seconds = 10
max_failure_rate_percent = 5.0
min_port_sync_rate = 90.0
enable_watchdog = true
watchdog_interval_secs = 15
```

### Custom Configuration

Create `/etc/sonic/portsyncd.conf`:

```toml
# Database connection
[database]
redis_host = "192.168.1.10"  # Redis server IP
redis_port = 6379             # Redis port
config_db_number = 4          # CONFIG_DB database number
state_db_number = 6           # STATE_DB database number

# Performance tuning
[performance]
max_event_queue = 5000        # Larger queue for burst handling
batch_timeout_ms = 50         # Faster batch processing
max_latency_us = 5000         # Stricter latency requirement

# Health monitoring
[health]
max_stall_seconds = 20        # Allow 20s without events
enable_watchdog = true        # Enable systemd watchdog
watchdog_interval_secs = 10   # Send watchdog every 10s
```

### Configuration Tuning Guide

**High-Latency Network** (>5ms RTT to Redis):

```toml
[performance]
batch_timeout_ms = 200
max_latency_us = 20000
```

**High-Throughput Scenario** (>5000 ports):

```toml
[performance]
max_event_queue = 10000
batch_timeout_ms = 50
```

**Memory-Constrained** (<2GB available):

```toml
[performance]
max_event_queue = 500
batch_timeout_ms = 100
```

**Reliability Priority** (never lose events):

```toml
[health]
max_failure_rate_percent = 1.0
min_port_sync_rate = 95.0
enable_watchdog = true
```

## Systemd Service Management

### Service File Explanation

```ini
[Service]
Type=notify
```

- Daemon signals readiness to systemd via sd_notify()
- systemd waits for READY signal before marking service ready

```ini
WatchdogSec=30s
```

- systemd restarts daemon if no WATCHDOG signal for 30 seconds
- Prevents daemon hangs from leaving system stuck

```ini
Restart=on-failure
RestartSec=5
StartLimitInterval=300s
StartLimitBurst=3
```

- Restart on crash (max 3 times in 5 minutes)
- Prevents restart loops

```ini
MemoryLimit=512M
MemoryAccounting=true
```

- Limit memory to 512MB
- Track memory usage for monitoring

### Customizing Service

Edit `/etc/systemd/system/portsyncd.service`:

```ini
[Service]
# Increase memory limit for very large deployments
MemoryLimit=1G

# Increase watchdog timeout for slower systems
WatchdogSec=60s

# Change log destination
StandardOutput=file:/var/log/portsyncd.log

# Run with higher priority
Nice=-5
CPUSchedulingPolicy=fifo
CPUSchedulingPriority=50
```

Reload and restart:

```bash
sudo systemctl daemon-reload
sudo systemctl restart portsyncd
```

## Monitoring

### System Health Check

```bash
# Is daemon running?
systemctl status portsyncd

# Recent logs
journalctl -u portsyncd -n 50

# Follow live logs
journalctl -u portsyncd -f

# Last boot logs
journalctl -u portsyncd -b
```

### Port Synchronization Status

```bash
# How many ports?
redis-cli -n 6 HLEN PORT_TABLE

# Check specific port
redis-cli -n 6 HGETALL 'PORT_TABLE|Ethernet0'

# Watch for updates
watch -n 1 'redis-cli -n 6 HGETALL PORT_TABLE | head -20'
```

### Performance Monitoring

```bash
# Check daemon memory
ps aux | grep portsyncd

# Monitor system resources
top -p $(pgrep portsyncd)

# Check disk usage
du -sh /var/log/portsyncd

# Monitor latency
journalctl -u portsyncd | grep latency_us
```

### Health Status

```bash
# Get daemon status
systemctl show portsyncd

# Expected output:
# ActiveState=active
# SubState=running
# StatusText=Healthy

# If unhealthy:
journalctl -u portsyncd | grep -i health
```

## Troubleshooting

### Daemon Won't Start

**Check logs**:

```bash
journalctl -u portsyncd -n 100
systemctl status portsyncd
```

**Common issues**:

1. **Redis not running**

   ```bash
   redis-cli PING
   # PONG = OK
   # Connection refused = Redis not running
   ```

   Fix: `systemctl start redis`

2. **Port already in use** (unlikely, but possible)

   ```bash
   sudo netstat -tlnp | grep 6379
   ```

3. **Config file syntax error**

   ```bash
   cat /etc/sonic/portsyncd.conf
   # Check TOML syntax
   ```

   Fix: Use online TOML validator

4. **Permission denied**

   ```bash
   ls -l /usr/local/bin/portsyncd
   # Should be: -rwxr-xr-x
   ```

   Fix: `sudo chmod 755 /usr/local/bin/portsyncd`

### High Event Latency

**Diagnose**:

```bash
# Check system load
top

# Check Redis latency
redis-cli --latency

# Check daemon CPU usage
ps aux | grep portsyncd
```

**Solutions**:

1. **Reduce system load**
   - Stop competing daemons
   - Check for background jobs: `jobs -l`

2. **Improve Redis performance**
   - Increase memory: `redis-cli CONFIG SET maxmemory 2gb`
   - Check for slow commands: `redis-cli SLOWLOG GET 10`

3. **Tune portsyncd**

   ```toml
   [performance]
   batch_timeout_ms = 50  # Process faster
   max_event_queue = 2000 # Larger buffer
   ```

### Memory Leak Suspected

**Check for leak**:

```bash
# Monitor memory for 24 hours
watch -n 3600 'ps aux | grep portsyncd | grep -v grep'

# Or use systemd
systemctl status portsyncd | watch grep Memory
```

**If memory growth detected**:

1. Check SONiC version for known leaks
2. Update to latest version: `cargo build --release`
3. Restart daemon: `systemctl restart portsyncd`
4. Monitor again

### Dropped Events

**Symptoms**: Port status not updating in STATE_DB

**Check**:

```bash
# Are events being received?
journalctl -u portsyncd | grep "Received event"

# Is Redis available?
redis-cli PING

# Check event queue size
redis-cli -n 6 LLEN portsyncd:queue
```

**Solutions**:

1. Increase queue size:

   ```toml
   [performance]
   max_event_queue = 5000
   ```

2. Check Redis memory:

   ```bash
   redis-cli INFO memory | grep used_memory_human
   ```

3. Reduce competing daemons

### Daemon Stuck/Unresponsive

**Check with watchdog**:

```bash
# systemd watchdog will restart it automatically after 30s
# Monitor restart
journalctl -u portsyncd | grep Restart

# Manual restart if needed
sudo systemctl restart portsyncd
```

**Force restart**:

```bash
# Kill daemon
sudo killall portsyncd

# Wait for systemd to restart
sleep 5
systemctl status portsyncd
```

## Upgrading

### Upgrade Steps

1. **Build new version**:

   ```bash
   cargo build --release
   ```

2. **Stop daemon**:

   ```bash
   sudo systemctl stop portsyncd
   ```

3. **Backup current binary**:

   ```bash
   sudo cp /usr/local/bin/portsyncd /usr/local/bin/portsyncd.bak
   ```

4. **Install new binary**:

   ```bash
   sudo cp target/release/portsyncd /usr/local/bin/
   ```

5. **Start daemon**:

   ```bash
   sudo systemctl start portsyncd
   ```

6. **Verify**:

   ```bash
   systemctl status portsyncd
   journalctl -u portsyncd -n 20
   ```

### Rollback if Needed

```bash
# Restore previous binary
sudo cp /usr/local/bin/portsyncd.bak /usr/local/bin/portsyncd

# Restart
sudo systemctl restart portsyncd
```

## Backup and Recovery

### Configuration Backup

```bash
# Backup config file
sudo cp /etc/sonic/portsyncd.conf /etc/sonic/portsyncd.conf.bak

# Backup systemd service
sudo cp /etc/systemd/system/portsyncd.service \
        /etc/systemd/system/portsyncd.service.bak

# Backup binary
sudo cp /usr/local/bin/portsyncd /usr/local/bin/portsyncd.bak
```

### Recovery

```bash
# Restore from backup
sudo cp /etc/sonic/portsyncd.conf.bak /etc/sonic/portsyncd.conf
sudo systemctl daemon-reload
sudo systemctl restart portsyncd
```

## Security Hardening

### Service Isolation

```ini
# In portsyncd.service
[Service]
PrivateTmp=true           # Private /tmp directory
NoNewPrivileges=true      # Can't gain new privileges
ProtectSystem=strict      # Read-only /usr, /etc
ProtectHome=yes           # No access to /root, /home
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
```

### Network Security

```ini
# Restrict to localhost only
[Service]
Environment="REDIS_HOST=127.0.0.1"

# Use netlink socket (kernel only)
# No external network access needed
```

### Resource Limits

```ini
[Service]
MemoryLimit=512M
MemoryAccounting=true
CPUQuota=50%            # Max 50% CPU
TasksMax=100            # Max 100 tasks
```

## Production Checklist

- [ ] Binary compiled with `--release`
- [ ] All 100+ tests pass: `cargo test --all-features`
- [ ] Load tests pass: >1000 eps, <10ms latency
- [ ] systemd service file installed
- [ ] Configuration file created (or using defaults)
- [ ] Redis running and accessible
- [ ] Daemon starts and reaches READY state
- [ ] Port synchronization working (check STATE_DB)
- [ ] Health monitoring active
- [ ] Logs flowing to journalctl
- [ ] Memory usage stable (<500MB)
- [ ] CPU usage normal (<10% at idle)
- [ ] Watchdog notifications working
- [ ] Graceful shutdown tested
- [ ] Monitoring and alerts configured
- [ ] Documentation updated for local deployment

## Monitoring Script

```bash
#!/bin/bash
# monitor_portsyncd.sh - Daily daemon health check

echo "=== portsyncd Health Check ==="
echo "Timestamp: $(date)"

# Status
echo ""
echo "Service Status:"
systemctl status portsyncd -n 0

# Performance
echo ""
echo "Recent Activity:"
journalctl -u portsyncd -n 10

# Resources
echo ""
echo "Resource Usage:"
ps aux | grep '[p]ortsyncd'

# Port Count
echo ""
echo "Port Synchronization:"
echo "Ports in STATE_DB: $(redis-cli -n 6 HLEN PORT_TABLE)"

# Errors
echo ""
echo "Recent Errors (last 100 lines):"
journalctl -u portsyncd -p err -n 100 || echo "No errors"

# Health
echo ""
echo "Health Status:"
systemctl show portsyncd | grep -E "(Active|Status|Memory)"
```

Run daily:

```bash
chmod +x monitor_portsyncd.sh
./monitor_portsyncd.sh | mail -s "portsyncd Health" admin@example.com
```

## References

- **systemd Service**: `man systemd.service`
- **Redis Protocol**: <https://redis.io/docs/>
- **SONiC Documentation**: <https://github.com/sonic-net/SONiC>
- **Netlink Sockets**: `man 7 netlink`

---

**Last Updated**: Phase 5 Week 5 (Production Deployment)
**Status**: Production Ready
**Test Coverage**: 100+ tests passing
**Performance**: <10ms latency, 1000+ eps throughput
