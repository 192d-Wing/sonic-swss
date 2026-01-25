# neighsyncd Production Deployment Guide

**Version:** 1.0
**Last Updated:** 2026-01-25
**Status:** Production Ready

## Table of Contents

1. [Overview](#overview)
2. [System Requirements](#system-requirements)
3. [Pre-Deployment Checklist](#pre-deployment-checklist)
4. [Build Instructions](#build-instructions)
5. [Certificate Setup (CNSA 2.0 mTLS)](#certificate-setup-cnsa-20-mtls)
6. [Installation](#installation)
7. [Configuration](#configuration)
8. [Systemd Integration](#systemd-integration)
9. [Starting and Stopping](#starting-and-stopping)
10. [Warm Restart Procedures](#warm-restart-procedures)
11. [Monitoring Setup](#monitoring-setup)
12. [Security Hardening](#security-hardening)
13. [Troubleshooting](#troubleshooting)
14. [Rollback Procedures](#rollback-procedures)

---

## Overview

neighsyncd is a production-grade network neighbor synchronization daemon for SONiC, written in Rust. It provides:

- **High-Performance Netlink Processing**: Async I/O with tokio runtime
- **Redis State Management**: Batched operations with pipelining
- **Warm Restart Support**: State caching and reconciliation
- **CNSA 2.0 Compliant mTLS**: Secure metrics endpoints
- **Prometheus Monitoring**: Comprehensive observability
- **Dual-ToR Support**: Multi-instance deployment capability

---

## System Requirements

### Hardware Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 2 cores | 4+ cores |
| Memory | 128 MB | 256 MB |
| Network | 1 Gbps | 10+ Gbps |

### Software Requirements

| Component | Version | Notes |
|-----------|---------|-------|
| Linux Kernel | 4.19+ | Netlink RTM_NEWNEIGH support required |
| Redis Server | 6.0+ | IPv6 support required |
| SONiC Version | 202211+ | Tested on Bullseye |
| Rust Toolchain | 1.75+ | Build-time only |
| OpenSSL/LibreSSL | 3.0+ | For certificate generation |

### Kernel Features

Verify required kernel features:

```bash
# Check netlink neighbor support
grep CONFIG_NETLINK_ROUTE /boot/config-$(uname -r)
# Should output: CONFIG_NETLINK_ROUTE=y

# Check IPv6 support
grep CONFIG_IPV6 /boot/config-$(uname -r)
# Should output: CONFIG_IPV6=y

# Check netfilter connection tracking (for state tracking)
grep CONFIG_NF_CONNTRACK /boot/config-$(uname -r)
# Should output: CONFIG_NF_CONNTRACK=y
```

---

## Pre-Deployment Checklist

### Before Deployment

- [ ] Redis server is running and accessible
- [ ] System meets minimum hardware requirements
- [ ] Kernel features verified
- [ ] Certificate infrastructure ready (CA, server certs, client certs)
- [ ] Prometheus monitoring stack available (if using metrics)
- [ ] Backup of existing neighsyncd configuration (if upgrading)
- [ ] Maintenance window scheduled (for production systems)
- [ ] Rollback plan documented

### Security Checklist

- [ ] CNSA 2.0 certificates generated (P-384 or P-521)
- [ ] Certificate private keys protected (0600 permissions)
- [ ] CA certificate distributed to Prometheus servers
- [ ] Firewall rules configured for metrics endpoint (port 9091)
- [ ] SELinux/AppArmor policies reviewed
- [ ] No world-readable configuration files

### Network Checklist

- [ ] Redis connectivity verified (IPv6 loopback [::1]:6379)
- [ ] Metrics endpoint accessible from monitoring hosts
- [ ] No conflicting services on port 9091
- [ ] IPv6 kernel routing enabled
- [ ] Netlink socket buffer size adequate (see tuning below)

---

## Build Instructions

### Development Build (Debug)

For testing and development with debug symbols:

```bash
cd /path/to/sonic-workspace/sonic-swss
cargo build -p sonic-neighsyncd

# Binary location:
# target/debug/sonic-neighsyncd
```

### Production Build (Release)

For production deployment with optimizations:

```bash
cd /path/to/sonic-workspace/sonic-swss
cargo build --release -p sonic-neighsyncd

# Binary location:
# target/release/sonic-neighsyncd

# Verify binary size (should be ~3-5 MB):
ls -lh target/release/sonic-neighsyncd

# Strip symbols for smaller size (optional):
strip target/release/sonic-neighsyncd
```

### Build with Specific Features

```bash
# Disable dual-ToR support (single-ToR deployment):
cargo build --release -p sonic-neighsyncd --no-default-features --features ipv6

# Enable only IPv4 support:
cargo build --release -p sonic-neighsyncd --no-default-features --features ipv4

# Both IPv4 and IPv6 (default):
cargo build --release -p sonic-neighsyncd
```

### Cross-Compilation (for ARM64 targets)

```bash
# Install cross-compilation toolchain:
rustup target add aarch64-unknown-linux-gnu

# Build for ARM64:
cargo build --release --target aarch64-unknown-linux-gnu -p sonic-neighsyncd
```

### Verify Build

```bash
# Check binary:
file target/release/sonic-neighsyncd
# Should output: ELF 64-bit LSB pie executable, x86-64...

# Test execution:
target/release/sonic-neighsyncd --version
# Should output: sonic-neighsyncd 1.0.0

# Run unit tests:
cargo test -p sonic-neighsyncd

# Run integration tests (requires Docker):
cargo test -p sonic-neighsyncd -- --ignored
```

---

## Certificate Setup (CNSA 2.0 mTLS)

### Overview

neighsyncd metrics endpoints require CNSA 2.0 compliant mTLS:

- **TLS Version**: TLS 1.3 only
- **Cipher Suite**: TLS_AES_256_GCM_SHA384 only
- **Key Exchange**: ECDHE with P-384 or P-521 curves
- **Certificates**: EC P-384+ with SHA-384+ signatures
- **Client Authentication**: Mandatory (mTLS)

### Certificate Authority Setup

Create a dedicated CA for neighsyncd metrics (if not already available):

```bash
# Create CA directory structure:
sudo mkdir -p /etc/sonic/metrics/ca
cd /etc/sonic/metrics/ca

# Generate EC P-384 CA private key:
openssl ecparam -name secp384r1 -genkey -noout -out ca-key.pem

# Protect CA private key:
sudo chmod 0400 ca-key.pem
sudo chown root:root ca-key.pem

# Generate CA certificate (valid 10 years):
openssl req -new -x509 -sha384 -key ca-key.pem -out ca-cert.pem -days 3650 \
  -subj "/C=US/ST=CA/L=San Francisco/O=SONiC/OU=Network Operations/CN=SONiC Metrics CA"

# Verify CA certificate:
openssl x509 -in ca-cert.pem -text -noout | grep -A2 "Public Key Algorithm"
# Should show: Public Key Algorithm: id-ecPublicKey, ASN1 OID: secp384r1
```

### Server Certificate Generation

Generate server certificate for neighsyncd metrics endpoint:

```bash
# Create server certificate directory:
sudo mkdir -p /etc/sonic/metrics/server
cd /etc/sonic/metrics/server

# Generate server EC P-384 private key:
openssl ecparam -name secp384r1 -genkey -noout -out server-key.pem

# Protect server private key:
sudo chmod 0600 server-key.pem
sudo chown sonic:sonic server-key.pem

# Create certificate signing request (CSR):
openssl req -new -sha384 -key server-key.pem -out server.csr \
  -subj "/C=US/ST=CA/L=San Francisco/O=SONiC/OU=neighsyncd/CN=neighsyncd-metrics"

# Create SAN extension file (critical for TLS 1.3):
cat > server-san.ext <<EOF
subjectAltName = IP:::1,DNS:localhost
extendedKeyUsage = serverAuth
keyUsage = digitalSignature, keyAgreement
EOF

# Sign server certificate with CA (valid 2 years):
openssl x509 -req -sha384 -in server.csr \
  -CA /etc/sonic/metrics/ca/ca-cert.pem \
  -CAkey /etc/sonic/metrics/ca/ca-key.pem \
  -CAcreateserial -out server-cert.pem -days 730 \
  -extfile server-san.ext

# Verify server certificate:
openssl verify -CAfile /etc/sonic/metrics/ca/ca-cert.pem server-cert.pem
# Should output: server-cert.pem: OK

# Clean up CSR and extension file:
rm server.csr server-san.ext
```

### Client Certificate Generation (for Prometheus)

Generate client certificate for Prometheus scraper:

```bash
# Create client certificate directory:
sudo mkdir -p /etc/sonic/metrics/clients/prometheus
cd /etc/sonic/metrics/clients/prometheus

# Generate client EC P-384 private key:
openssl ecparam -name secp384r1 -genkey -noout -out client-key.pem

# Protect client private key:
sudo chmod 0600 client-key.pem

# Create client CSR:
openssl req -new -sha384 -key client-key.pem -out client.csr \
  -subj "/C=US/ST=CA/L=San Francisco/O=SONiC/OU=Monitoring/CN=prometheus-scraper"

# Create client extension file:
cat > client-ext.ext <<EOF
extendedKeyUsage = clientAuth
keyUsage = digitalSignature
EOF

# Sign client certificate with CA:
openssl x509 -req -sha384 -in client.csr \
  -CA /etc/sonic/metrics/ca/ca-cert.pem \
  -CAkey /etc/sonic/metrics/ca/ca-key.pem \
  -CAcreateserial -out client-cert.pem -days 730 \
  -extfile client-ext.ext

# Verify client certificate:
openssl verify -CAfile /etc/sonic/metrics/ca/ca-cert.pem client-cert.pem
# Should output: client-cert.pem: OK

# Clean up:
rm client.csr client-ext.ext
```

### Certificate Permissions

```bash
# Set correct ownership and permissions:
sudo chown -R sonic:sonic /etc/sonic/metrics/server
sudo chown -R sonic:sonic /etc/sonic/metrics/clients
sudo chmod 0755 /etc/sonic/metrics/{ca,server,clients}
sudo chmod 0644 /etc/sonic/metrics/ca/ca-cert.pem
sudo chmod 0644 /etc/sonic/metrics/server/server-cert.pem
sudo chmod 0600 /etc/sonic/metrics/server/server-key.pem
sudo chmod 0644 /etc/sonic/metrics/clients/prometheus/client-cert.pem
sudo chmod 0600 /etc/sonic/metrics/clients/prometheus/client-key.pem
```

### Certificate Verification

Verify CNSA 2.0 compliance:

```bash
# Verify server certificate uses P-384:
openssl x509 -in /etc/sonic/metrics/server/server-cert.pem -text -noout | \
  grep "Public Key Algorithm" -A2
# Should show: id-ecPublicKey, ASN1 OID: secp384r1

# Verify SHA-384 signature:
openssl x509 -in /etc/sonic/metrics/server/server-cert.pem -text -noout | \
  grep "Signature Algorithm"
# Should show: ecdsa-with-SHA384

# Test mTLS connection:
openssl s_client -connect [::1]:9091 \
  -CAfile /etc/sonic/metrics/ca/ca-cert.pem \
  -cert /etc/sonic/metrics/clients/prometheus/client-cert.pem \
  -key /etc/sonic/metrics/clients/prometheus/client-key.pem \
  -tls1_3 -ciphersuites TLS_AES_256_GCM_SHA384
# Should output: Verify return code: 0 (ok)
```

---

## Installation

### Binary Installation

```bash
# Copy binary to system location:
sudo cp target/release/sonic-neighsyncd /usr/local/bin/
sudo chmod 0755 /usr/local/bin/sonic-neighsyncd
sudo chown root:root /usr/local/bin/sonic-neighsyncd

# Verify installation:
/usr/local/bin/sonic-neighsyncd --version
```

### Directory Structure

```bash
# Create runtime directories:
sudo mkdir -p /etc/sonic/neighsyncd
sudo mkdir -p /var/log/sonic/neighsyncd
sudo mkdir -p /var/run/sonic/neighsyncd

# Set ownership:
sudo chown sonic:sonic /etc/sonic/neighsyncd
sudo chown sonic:sonic /var/log/sonic/neighsyncd
sudo chown sonic:sonic /var/run/sonic/neighsyncd

# Set permissions:
sudo chmod 0750 /etc/sonic/neighsyncd
sudo chmod 0755 /var/log/sonic/neighsyncd
sudo chmod 0755 /var/run/sonic/neighsyncd
```

### User and Group

neighsyncd should run as a dedicated non-root user:

```bash
# Create sonic user (if not already exists):
sudo useradd -r -s /bin/false -d /var/run/sonic -c "SONiC Daemon User" sonic

# Add to required groups:
sudo usermod -a -G adm sonic   # For log access
```

---

## Configuration

### Default Configuration

neighsyncd uses sensible defaults and requires minimal configuration:

```bash
# Create configuration file:
sudo tee /etc/sonic/neighsyncd/neighsyncd.conf > /dev/null <<'EOF'
# neighsyncd configuration file
# See CONFIGURATION.md for detailed options

[redis]
host = "::1"
port = 6379
database = 0

[netlink]
socket_buffer_size = 262144  # 256 KB
timeout_ms = 5000

[logging]
level = "info"
format = "json"

[performance]
batch_size = 100
reconcile_timeout_ms = 5000

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
EOF

# Set permissions:
sudo chown sonic:sonic /etc/sonic/neighsyncd/neighsyncd.conf
sudo chmod 0640 /etc/sonic/neighsyncd/neighsyncd.conf
```

### Environment Variables

Alternative to configuration file:

```bash
# Redis configuration:
export NEIGHSYNCD_REDIS_HOST="::1"
export NEIGHSYNCD_REDIS_PORT="6379"

# Logging:
export NEIGHSYNCD_LOG_LEVEL="info"

# Metrics:
export NEIGHSYNCD_METRICS_PORT="9091"
```

---

## Systemd Integration

### Service File Installation

```bash
# Copy systemd service file:
sudo cp crates/neighsyncd/neighsyncd.service /etc/systemd/system/

# Reload systemd:
sudo systemctl daemon-reload

# Verify service file:
systemd-analyze verify neighsyncd.service
# Should output nothing if valid
```

### Service File Contents

Located at `/etc/systemd/system/neighsyncd.service`:

```ini
[Unit]
Description=SONiC Neighbor Sync Daemon (Rust)
Documentation=https://github.com/sonic-net/sonic-swss
After=network-online.target redis.service
Wants=network-online.target
Requires=redis.service

[Service]
Type=notify
NotifyAccess=main
User=sonic
Group=sonic
Environment="RUST_LOG=info"
ExecStart=/usr/local/bin/sonic-neighsyncd
Restart=on-failure
RestartSec=5s
WatchdogSec=15s
TimeoutStartSec=30s
TimeoutStopSec=10s

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/sonic/neighsyncd /var/run/sonic/neighsyncd
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictRealtime=true
RestrictNamespaces=true
LockPersonality=true
MemoryDenyWriteExecute=true
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6 AF_NETLINK

# Resource limits
MemoryLimit=256M
MemoryHigh=200M
TasksMax=32
LimitNOFILE=65536

# Capabilities (minimal required for netlink)
CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_RAW
AmbientCapabilities=CAP_NET_ADMIN CAP_NET_RAW

[Install]
WantedBy=multi-user.target
```

### Enable and Start Service

```bash
# Enable on boot:
sudo systemctl enable neighsyncd.service

# Start service:
sudo systemctl start neighsyncd.service

# Check status:
sudo systemctl status neighsyncd.service

# View logs:
sudo journalctl -u neighsyncd.service -f
```

---

## Starting and Stopping

### Manual Start

```bash
# Start with systemd (recommended):
sudo systemctl start neighsyncd.service

# Direct execution (for testing):
sudo -u sonic /usr/local/bin/sonic-neighsyncd
```

### Graceful Shutdown

```bash
# Graceful stop (SIGTERM):
sudo systemctl stop neighsyncd.service

# Force kill (if unresponsive):
sudo systemctl kill -s SIGKILL neighsyncd.service

# Restart:
sudo systemctl restart neighsyncd.service
```

### Verification

```bash
# Check if running:
systemctl is-active neighsyncd.service

# View process:
ps aux | grep sonic-neighsyncd

# Check listening ports:
sudo ss -tlnp | grep 9091
# Should show: [::1]:9091 LISTEN ... sonic-neighsyncd

# Verify Redis connection:
redis-cli -h ::1 -p 6379 CLIENT LIST | grep neighsyncd
```

---

## Warm Restart Procedures

### Overview

Warm restart allows neighsyncd to cache state before restart and reconcile with kernel state after restart, minimizing service disruption.

### Initiating Warm Restart

```bash
# 1. Enable warm restart mode in Redis:
redis-cli -h ::1 SET "WARM_RESTART_ENABLE_TABLE|neighsyncd" "true"

# 2. Trigger graceful shutdown:
sudo systemctl stop neighsyncd.service

# At this point, neighsyncd will:
# - Cache current neighbor state to Redis
# - Stop processing new events
# - Exit gracefully
```

### Post-Restart Reconciliation

```bash
# 3. Start neighsyncd:
sudo systemctl start neighsyncd.service

# neighsyncd will automatically:
# - Detect warm restart mode
# - Load cached state from Redis
# - Wait for 5-second reconciliation timer
# - Query kernel for current neighbor state
# - Reconcile differences (add/update/delete)
# - Resume normal operation

# 4. Monitor reconciliation:
sudo journalctl -u neighsyncd.service -f | grep -i "warm restart"

# Expected log sequence:
# INFO neighsyncd: Warm restart mode detected
# INFO neighsyncd: Loaded 1234 cached neighbors from Redis
# INFO neighsyncd: Starting reconciliation timer (5000ms)
# INFO neighsyncd: Querying kernel neighbor state
# INFO neighsyncd: Reconciliation complete: 50 added, 10 updated, 5 deleted
# INFO neighsyncd: Warm restart finished, resuming normal operation
```

### Verification

```bash
# Check warm restart metrics:
curl -k --cert /etc/sonic/metrics/clients/prometheus/client-cert.pem \
     --key /etc/sonic/metrics/clients/prometheus/client-key.pem \
     https://[::1]:9091/metrics | grep warm_restart

# Verify neighbor count matches:
ip -6 neigh show | wc -l  # Kernel neighbor count
redis-cli -h ::1 KEYS "NEIGH_TABLE:*" | wc -l  # Redis neighbor count
```

### Disable Warm Restart

```bash
# Remove warm restart flag:
redis-cli -h ::1 DEL "WARM_RESTART_ENABLE_TABLE|neighsyncd"

# Next restart will be cold start (state cleared)
```

---

## Monitoring Setup

### Prometheus Integration

Configure Prometheus to scrape neighsyncd metrics with mTLS:

```yaml
# Add to prometheus.yml:
scrape_configs:
  - job_name: 'neighsyncd'
    scheme: https
    tls_config:
      ca_file: /etc/sonic/metrics/ca/ca-cert.pem
      cert_file: /etc/sonic/metrics/clients/prometheus/client-cert.pem
      key_file: /etc/sonic/metrics/clients/prometheus/client-key.pem
      server_name: 'neighsyncd-metrics'
    static_configs:
      - targets: ['[::1]:9091']
        labels:
          instance: 'sonic-switch-01'
          datacenter: 'dc1'
```

### Alert Rules

Create `/etc/prometheus/rules/neighsyncd.yaml`:

```yaml
groups:
  - name: neighsyncd_alerts
    interval: 30s
    rules:
      - alert: NeighsyncdHighErrorRate
        expr: rate(neighsyncd_events_failed_total[5m]) > 0.01
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "neighsyncd high error rate on {{ $labels.instance }}"
          description: "Error rate: {{ $value | humanizePercentage }}"

      - alert: NeighsyncdRedisDisconnected
        expr: neighsyncd_redis_connected == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "neighsyncd Redis disconnected on {{ $labels.instance }}"

      - alert: NeighsyncdHighMemory
        expr: neighsyncd_memory_bytes > 200000000
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "neighsyncd high memory usage on {{ $labels.instance }}"
          description: "Memory: {{ $value | humanize1024 }}B"

      - alert: NeighsyncdHighLatency
        expr: histogram_quantile(0.99, rate(neighsyncd_event_latency_seconds_bucket[5m])) > 0.1
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "neighsyncd high P99 latency on {{ $labels.instance }}"
          description: "P99 latency: {{ $value }}s"
```

### Grafana Dashboard

Import dashboard from `crates/neighsyncd/dashboards/neighsyncd.json` or create panels:

- **Neighbor Throughput**: `rate(neighsyncd_neighbors_processed_total[5m])`
- **Error Rate**: `rate(neighsyncd_events_failed_total[5m])`
- **P99 Latency**: `histogram_quantile(0.99, rate(neighsyncd_event_latency_seconds_bucket[5m]))`
- **Memory Usage**: `neighsyncd_memory_bytes`
- **Health Status**: `neighsyncd_health_status`

---

## Security Hardening

### Systemd Security Features

Already included in service file:

- `NoNewPrivileges=true` - Prevent privilege escalation
- `PrivateTmp=true` - Isolated /tmp
- `ProtectSystem=strict` - Read-only root filesystem
- `ProtectHome=true` - Hide /home
- `ProtectKernelTunables=true` - Protect /proc/sys
- `MemoryDenyWriteExecute=true` - No W^X memory
- `CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_RAW` - Minimal capabilities

### Firewall Configuration

Restrict metrics endpoint access:

```bash
# Allow only from monitoring server (iptables):
sudo ip6tables -A INPUT -p tcp --dport 9091 -s 2001:db8::monitoring -j ACCEPT
sudo ip6tables -A INPUT -p tcp --dport 9091 -j DROP

# Or use firewalld:
sudo firewall-cmd --permanent --add-rich-rule='
  rule family="ipv6"
  source address="2001:db8::monitoring"
  port port="9091" protocol="tcp"
  accept'
sudo firewall-cmd --reload
```

### SELinux Policy

If running SELinux:

```bash
# Generate custom policy (if needed):
sudo ausearch -c 'sonic-neighsyncd' --raw | audit2allow -M neighsyncd-policy
sudo semodule -i neighsyncd-policy.pp

# Or use permissive mode for testing:
sudo semanage permissive -a sonic_neighsyncd_t
```

### File Integrity Monitoring

Monitor critical files:

```bash
# Add to AIDE or Tripwire configuration:
/usr/local/bin/sonic-neighsyncd p+i+n+u+g+s+b+m+c+sha256
/etc/sonic/neighsyncd/neighsyncd.conf p+i+n+u+g+s+b+m+c+sha256
/etc/sonic/metrics/server/server-key.pem p+i+n+u+g+s+b+m+c+sha256
```

---

## Troubleshooting

### Service Won't Start

```bash
# Check systemd logs:
sudo journalctl -u neighsyncd.service -n 50

# Common issues:
# 1. Redis not running:
sudo systemctl status redis.service

# 2. Certificate permissions:
ls -l /etc/sonic/metrics/server/
sudo chown sonic:sonic /etc/sonic/metrics/server/server-key.pem
sudo chmod 0600 /etc/sonic/metrics/server/server-key.pem

# 3. Port already in use:
sudo ss -tlnp | grep 9091
```

### High Memory Usage

```bash
# Check current memory:
ps aux | grep sonic-neighsyncd

# View metrics:
curl -k --cert /etc/sonic/metrics/clients/prometheus/client-cert.pem \
     --key /etc/sonic/metrics/clients/prometheus/client-key.pem \
     https://[::1]:9091/metrics | grep memory_bytes

# Investigate neighbor table size:
redis-cli -h ::1 KEYS "NEIGH_TABLE:*" | wc -l
```

### Redis Connection Failures

```bash
# Test Redis connectivity:
redis-cli -h ::1 -p 6379 PING

# Check Redis logs:
sudo journalctl -u redis.service -f

# Verify Redis IPv6 binding:
redis-cli CONFIG GET bind
```

### Metrics Endpoint Unreachable

```bash
# Test mTLS connection:
openssl s_client -connect [::1]:9091 \
  -CAfile /etc/sonic/metrics/ca/ca-cert.pem \
  -cert /etc/sonic/metrics/clients/prometheus/client-cert.pem \
  -key /etc/sonic/metrics/clients/prometheus/client-key.pem \
  -tls1_3

# Check certificate expiration:
openssl x509 -in /etc/sonic/metrics/server/server-cert.pem -noout -dates

# Verify service is listening:
sudo ss -tlnp | grep 9091
```

For comprehensive troubleshooting, see [TROUBLESHOOTING.md](TROUBLESHOOTING.md).

---

## Rollback Procedures

### Emergency Rollback

If neighsyncd fails after deployment:

```bash
# 1. Stop new version:
sudo systemctl stop neighsyncd.service

# 2. Restore previous binary:
sudo cp /usr/local/bin/sonic-neighsyncd.backup /usr/local/bin/sonic-neighsyncd

# 3. Restore previous configuration:
sudo cp /etc/sonic/neighsyncd/neighsyncd.conf.backup /etc/sonic/neighsyncd/neighsyncd.conf

# 4. Start previous version:
sudo systemctl start neighsyncd.service

# 5. Verify:
sudo systemctl status neighsyncd.service
```

### Pre-Upgrade Backup

Before deploying new version:

```bash
# Backup binary:
sudo cp /usr/local/bin/sonic-neighsyncd /usr/local/bin/sonic-neighsyncd.backup

# Backup configuration:
sudo cp /etc/sonic/neighsyncd/neighsyncd.conf /etc/sonic/neighsyncd/neighsyncd.conf.backup

# Backup Redis state (optional):
redis-cli -h ::1 --rdb /tmp/neighsyncd-backup.rdb
```

---

## Production Deployment Checklist

### Pre-Deployment

- [ ] All tests pass (`cargo test -p sonic-neighsyncd`)
- [ ] Integration tests pass (`cargo test -p sonic-neighsyncd -- --ignored`)
- [ ] Benchmarks baseline established
- [ ] Certificates generated and verified
- [ ] Configuration file created and reviewed
- [ ] Systemd service file installed
- [ ] Firewall rules configured
- [ ] Prometheus scrape config updated
- [ ] Alert rules deployed
- [ ] Rollback plan documented
- [ ] Maintenance window scheduled

### Deployment

- [ ] Binary installed to `/usr/local/bin/`
- [ ] Service enabled: `systemctl enable neighsyncd.service`
- [ ] Service started: `systemctl start neighsyncd.service`
- [ ] Health check passed: `systemctl is-active neighsyncd.service`
- [ ] Metrics endpoint accessible
- [ ] Prometheus scraping successfully
- [ ] No errors in logs: `journalctl -u neighsyncd.service`

### Post-Deployment

- [ ] Neighbor synchronization verified (Redis vs kernel)
- [ ] Metrics dashboards showing data
- [ ] Alert rules firing correctly (test with known issue)
- [ ] Performance metrics within baseline
- [ ] Memory usage stable
- [ ] No security audit findings
- [ ] Documentation updated

---

## Support and Resources

- **GitHub Repository**: https://github.com/sonic-net/sonic-swss
- **Documentation**: `docs/rust/neighsyncd/`
- **Issue Tracker**: https://github.com/sonic-net/sonic-swss/issues
- **Security Contact**: security@sonic-net.org

---

**End of Deployment Guide**
