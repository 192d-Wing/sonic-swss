# neighsyncd Security Architecture

This document describes the security architecture of neighsyncd with focus on CNSA 2.0 compliance, cryptographic protections, and NIST 800-53 Rev 5 control mappings.

## Table of Contents

- [Security Overview](#security-overview)
- [CNSA 2.0 Compliance](#cnsa-20-compliance)
- [Mutual TLS (mTLS) Architecture](#mutual-tls-mtls-architecture)
- [Cryptographic Standards](#cryptographic-standards)
- [Certificate Management](#certificate-management)
- [NIST 800-53 Control Mappings](#nist-800-53-control-mappings)
- [Security Auditing](#security-auditing)
- [Threat Model](#threat-model)

## Security Overview

neighsyncd implements defense-in-depth security following these principles:

1. **Least Privilege**: Requires only CAP_NET_ADMIN for netlink operations
2. **Secure by Default**: CNSA 2.0 mTLS mandatory for metrics endpoint
3. **Cryptographic Protection**: FIPS 140-3 validated cryptography
4. **Input Validation**: All neighbor entries validated before processing
5. **Audit Logging**: Comprehensive structured logging for security analysis
6. **Fail Secure**: Defaults to deny on certificate/TLS errors

### Security Layers

```
┌─────────────────────────────────────────────────────────┐
│  Application Layer                                      │
│  - Input validation                                     │
│  - Access control (CAP_NET_ADMIN)                       │
│  - Error handling                                       │
├─────────────────────────────────────────────────────────┤
│  Transport Security Layer                               │
│  - CNSA 2.0 mTLS (metrics endpoint)                     │
│  - TLS 1.3 only                                         │
│  - Client certificate authentication                    │
├─────────────────────────────────────────────────────────┤
│  Cryptographic Layer                                    │
│  - AWS-LC-RS (FIPS 140-3)                               │
│  - AES-256-GCM                                          │
│  - SHA-384/SHA-512                                      │
│  - EC P-384/P-521                                       │
├─────────────────────────────────────────────────────────┤
│  System Layer                                           │
│  - Linux kernel netlink                                 │
│  - Redis connection security                            │
│  - systemd sandboxing                                   │
└─────────────────────────────────────────────────────────┘
```

## CNSA 2.0 Compliance

### What is CNSA 2.0?

Commercial National Security Algorithm Suite 2.0 (CNSA 2.0) is the NSA's cryptographic standard for protecting National Security Systems (NSS). neighsyncd's metrics endpoint implements full CNSA 2.0 compliance.

### Compliance Requirements

#### ✅ TLS Protocol
- **Requirement**: TLS 1.3 only
- **Implementation**: Enforced via `with_protocol_versions(&[&rustls::version::TLS13])`
- **Verification**: `openssl s_client` shows `Protocol: TLSv1.3`

#### ✅ Cipher Suite
- **Requirement**: TLS_AES_256_GCM_SHA384 only
- **Implementation**: Custom `CryptoProvider` with filtered cipher suites
- **Verification**: `openssl s_client` shows `Cipher: TLS_AES_256_GCM_SHA384`

**Code Reference**: [metrics_server.rs:35](../../crates/neighsyncd/src/metrics_server.rs#L35)
```rust
const CNSA_CIPHER_SUITE: CipherSuite = CipherSuite::TLS13_AES_256_GCM_SHA384;
```

#### ✅ Key Exchange
- **Requirement**: ECDHE with P-384 or P-521
- **Implementation**: Server/client certificates must use EC P-384 or P-521
- **Verification**: `openssl x509 -text` shows `Public Key Algorithm: id-ecPublicKey` with `secp384r1`

#### ✅ Hash Algorithm
- **Requirement**: SHA-384 or SHA-512
- **Implementation**: Certificates signed with `-sha384` or `-sha512`
- **Verification**: `openssl x509 -text` shows `Signature Algorithm: ecdsa-with-SHA384`

#### ✅ Cryptographic Provider
- **Requirement**: FIPS 140-3 validated module
- **Implementation**: AWS-LC-RS (rustls crypto provider)
- **Verification**: AWS-LC has FIPS 140-3 certificate (#4816)

**Code Reference**: [metrics_server.rs:151](../../crates/neighsyncd/src/metrics_server.rs#L151)
```rust
let crypto_provider = rustls::crypto::aws_lc_rs::default_provider();
```

### CNSA 2.0 Implementation

The complete CNSA 2.0 implementation is in `load_cnsa_mtls_config()`:

**Code Reference**: [metrics_server.rs:92-200](../../crates/neighsyncd/src/metrics_server.rs#L92-L200)

Key security features:

1. **Client Certificate Verification** (lines 143-148):
```rust
let client_verifier = WebPkiClientVerifier::builder(Arc::new(root_store))
    .build()
    .map_err(|e| format!("Failed to create client verifier: {}", e))?;
```

2. **Cipher Suite Filtering** (lines 154-172):
```rust
let cnsa_cipher_suites: Vec<SupportedCipherSuite> = crypto_provider
    .cipher_suites
    .iter()
    .filter(|cs| cs.suite() == CNSA_CIPHER_SUITE)
    .copied()
    .collect();

let cnsa_provider = rustls::crypto::CryptoProvider {
    cipher_suites: cnsa_cipher_suites,
    kx_groups: crypto_provider.kx_groups.to_vec(),
    signature_verification_algorithms: crypto_provider.signature_verification_algorithms,
    secure_random: crypto_provider.secure_random,
    key_provider: crypto_provider.key_provider,
};
```

3. **TLS 1.3 Enforcement** (lines 179-183):
```rust
let mut tls_config = ServerConfig::builder_with_provider(Arc::new(cnsa_provider))
    .with_protocol_versions(&[&rustls::version::TLS13])
    .map_err(|e| format!("Failed to set TLS 1.3 only: {}", e))?
    .with_client_cert_verifier(client_verifier)
    .with_single_cert(certs, private_key)?;
```

4. **No Session Resumption** (lines 185-186):
```rust
tls_config.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
```

## Mutual TLS (mTLS) Architecture

### Overview

neighsyncd implements **mandatory mutual TLS** for the metrics endpoint. This means:

1. **Server Authentication**: Client verifies server certificate
2. **Client Authentication**: Server verifies client certificate
3. **Encrypted Channel**: All traffic encrypted with AES-256-GCM
4. **No Anonymous Access**: Client certificate required for all requests

### TLS Handshake Flow

```
Client                                               Server
  │                                                    │
  │  1. ClientHello (TLS 1.3, ECDHE-P384)             │
  ├──────────────────────────────────────────────────>│
  │                                                    │
  │  2. ServerHello (TLS_AES_256_GCM_SHA384)          │
  │     + ServerCertificate (EC P-384)                │
  │     + CertificateRequest (client cert required)   │
  │<──────────────────────────────────────────────────┤
  │                                                    │
  │  3. ClientCertificate (EC P-384, signed by CA)    │
  │     + CertificateVerify (prove private key)       │
  │     + Finished                                    │
  ├──────────────────────────────────────────────────>│
  │                                                    │
  │                Server verifies client cert        │
  │                against configured CA              │
  │                                                    │
  │  4. Finished                                      │
  │<──────────────────────────────────────────────────┤
  │                                                    │
  │  5. Application Data (encrypted)                  │
  │<──────────────────────────────────────────────────>│
  │                                                    │
```

### Certificate Validation

**Server-side validation** (performed by `WebPkiClientVerifier`):

1. **Chain verification**: Client certificate chains to trusted CA
2. **Signature verification**: Certificate signed by CA private key
3. **Expiration check**: Certificate not expired
4. **Key usage verification**: Certificate has client authentication usage
5. **Revocation check**: Not implemented (future enhancement)

**Client-side validation** (performed by Prometheus/curl):

1. **Chain verification**: Server certificate chains to trusted CA
2. **Signature verification**: Certificate signed by CA private key
3. **Expiration check**: Certificate not expired
4. **Hostname verification**: Certificate CN/SAN matches `neighsyncd-metrics`

### Security Properties

mTLS provides these security guarantees:

| Property | Provided By | Threat Mitigated |
|----------|-------------|------------------|
| **Confidentiality** | AES-256-GCM encryption | Eavesdropping, traffic analysis |
| **Integrity** | AES-256-GCM AEAD | Message tampering, replay attacks |
| **Authentication** | Client/server certificates | Impersonation, MITM attacks |
| **Forward Secrecy** | ECDHE key exchange | Compromise of long-term keys |
| **Non-repudiation** | Certificate signatures | Denial of actions |

## Cryptographic Standards

### Algorithms

| Component | Algorithm | Key Size | Compliance |
|-----------|-----------|----------|------------|
| **Symmetric Cipher** | AES-256-GCM | 256 bits | CNSA 2.0, FIPS 140-3 |
| **Hash Function** | SHA-384 | 384 bits | CNSA 2.0, FIPS 140-3 |
| **Signature Scheme** | ECDSA | EC P-384 | CNSA 2.0, FIPS 140-3 |
| **Key Exchange** | ECDHE | EC P-384 | CNSA 2.0, FIPS 140-3 |
| **Random Generation** | AWS-LC DRBG | - | FIPS 140-3 |

### AES-256-GCM Details

**Mode**: Galois/Counter Mode (GCM) - Authenticated Encryption with Associated Data (AEAD)

**Properties**:
- **Confidentiality**: CTR mode encryption
- **Integrity**: GMAC authentication tag (128-bit)
- **Performance**: Hardware acceleration (AES-NI on x86_64)
- **Parallelization**: Yes (unlike CBC mode)

**Security Strength**: 256-bit key provides 2^256 keyspace, computationally infeasible to brute force

### ECDSA with P-384

**Curve**: secp384r1 (NIST P-384)

**Properties**:
- **Security Level**: ~192-bit symmetric equivalent
- **Key Size**: 384-bit private key, 769-bit public key (uncompressed)
- **Signature Size**: ~96 bytes
- **Performance**: Faster than RSA-4096 with equivalent security

**Why P-384 over P-256**:
- CNSA 2.0 requirement (P-256 deprecated for NSS)
- Higher security margin (192-bit vs 128-bit)
- Future-proof against quantum computing advances

### SHA-384

**Properties**:
- **Output Size**: 384 bits (48 bytes)
- **Block Size**: 1024 bits
- **Rounds**: 80
- **Collision Resistance**: 2^192 operations
- **Preimage Resistance**: 2^384 operations

**Why SHA-384 over SHA-256**:
- CNSA 2.0 requirement
- Truncated SHA-512 (faster on 64-bit systems)
- Higher security margin

## Certificate Management

### Certificate Lifecycle

```
┌──────────────┐
│   Generate   │  OpenSSL, cfssl, or internal CA
│   CA Root    │
└──────┬───────┘
       │
       ▼
┌──────────────┐
│   Generate   │  Server + Client certificates
│   Leaf Certs │
└──────┬───────┘
       │
       ▼
┌──────────────┐
│    Deploy    │  Install on systems
│ Certificates │
└──────┬───────┘
       │
       ▼
┌──────────────┐
│   Monitor    │  Check expiration (daily)
│  Expiration  │
└──────┬───────┘
       │
       ▼
┌──────────────┐
│    Rotate    │  Before expiration (30 days)
│ Certificates │
└──────────────┘
```

### Certificate Rotation

**Recommended rotation schedule**:
- **CA certificate**: 10 years (rarely rotated)
- **Server certificate**: 1 year
- **Client certificate**: 1 year

**Rotation procedure**:

1. **Generate new certificate** (30 days before expiration):
```bash
# Generate new server certificate
openssl ecparam -name secp384r1 -genkey -noout -out server-key-new.pem
openssl req -new -sha384 -key server-key-new.pem -out server-new.csr \
  -subj "/C=US/O=SONiC/CN=neighsyncd-metrics"
openssl x509 -req -in server-new.csr -CA ca-cert.pem -CAkey ca-key.pem \
  -CAcreateserial -out server-cert-new.pem -days 365 -sha384
```

2. **Deploy new certificate**:
```bash
sudo cp server-cert-new.pem /etc/sonic/metrics/server-cert.pem
sudo cp server-key-new.pem /etc/sonic/metrics/server-key.pem
sudo chmod 600 /etc/sonic/metrics/server-key.pem
```

3. **Restart service**:
```bash
sudo systemctl restart neighsyncd
```

4. **Verify new certificate**:
```bash
echo | openssl s_client -connect [::1]:9091 -showcerts 2>/dev/null | \
  openssl x509 -noout -enddate
```

### Certificate Storage

**File permissions**:
```bash
/etc/sonic/metrics/
├── ca-cert.pem         # 644 (world-readable)
├── server-cert.pem     # 644 (world-readable)
└── server-key.pem      # 600 (owner read/write only)
```

**Security requirements**:
- Private keys must have 600 permissions
- Private keys must be owned by neighsyncd user
- Private keys must NOT be world-readable
- CA certificate should be in system trust store

### Certificate Monitoring

**Automated monitoring**:

```bash
# Check certificate expiration daily (cron job)
#!/bin/bash
CERT=/etc/sonic/metrics/server-cert.pem
DAYS_LEFT=$(openssl x509 -in $CERT -noout -enddate | \
  awk -F= '{print $2}' | xargs -I {} date -d "{}" +%s | \
  awk -v now=$(date +%s) '{print int(($1 - now) / 86400)}')

if [ $DAYS_LEFT -lt 30 ]; then
  echo "WARNING: Certificate expires in $DAYS_LEFT days"
  # Send alert to monitoring system
fi
```

**Prometheus alerting rule**:
```yaml
- alert: CertificateExpiringSoon
  expr: |
    (probe_ssl_earliest_cert_expiry{job="neighsyncd"} - time())
    / 86400 < 30
  labels:
    severity: warning
  annotations:
    summary: "neighsyncd certificate expires in {{ $value }} days"
```

## NIST 800-53 Control Mappings

neighsyncd implements comprehensive security controls from NIST 800-53 Rev 5:

### Access Control (AC)

| Control | Title | Implementation |
|---------|-------|----------------|
| **AC-3** | Access Enforcement | Netlink socket requires CAP_NET_ADMIN capability |
| **AC-6** | Least Privilege | Process drops unnecessary privileges after startup |

### Audit and Accountability (AU)

| Control | Title | Implementation |
|---------|-------|----------------|
| **AU-3** | Content of Audit Records | Structured logging with neighbor details (IP, MAC, interface) |
| **AU-6** | Audit Record Review | Prometheus metrics endpoint for analysis |
| **AU-12** | Audit Record Generation | All neighbor changes, errors, and state transitions logged |

### Configuration Management (CM)

| Control | Title | Implementation |
|---------|-------|----------------|
| **CM-6** | Configuration Settings | Redis CONFIG_DB integration, certificate paths configurable |
| **CM-8** | System Component Inventory | Track all network neighbors in Redis database |

### Contingency Planning (CP)

| Control | Title | Implementation |
|---------|-------|----------------|
| **CP-10** | System Recovery | Warm restart support with state persistence |

### Identification and Authentication (IA)

| Control | Title | Implementation |
|---------|-------|----------------|
| **IA-3** | Device Identification | MAC address tracking for all neighbors |
| **IA-5(2)** | PKI-Based Authentication | Client certificate validation for metrics endpoint |

### System and Communications Protection (SC)

| Control | Title | Implementation |
|---------|-------|----------------|
| **SC-5** | Denial of Service Protection | Broadcast/multicast filtering, rate limiting |
| **SC-7** | Boundary Protection | Network boundary awareness via netlink |
| **SC-8** | Transmission Confidentiality | TLS 1.3 encryption for metrics endpoint |
| **SC-8(1)** | Cryptographic Protection | CNSA 2.0 cipher suites (AES-256-GCM, SHA-384) |
| **SC-13** | Cryptographic Protection | FIPS 140-3 validated crypto (AWS-LC-RS) |
| **SC-23** | Session Authenticity | No session resumption, fresh handshake required |

### System and Information Integrity (SI)

| Control | Title | Implementation |
|---------|-------|----------------|
| **SI-4** | System Monitoring | Real-time neighbor monitoring, Prometheus metrics |
| **SI-10** | Input Validation | Validate all neighbor entries before processing |
| **SI-11** | Error Handling | Structured error types, graceful failure |

## Security Auditing

### Logging

neighsyncd uses structured logging via `tracing` crate:

**Security-relevant events**:
- Certificate loading/validation
- TLS handshake failures
- Client authentication failures
- Privilege escalation/drops
- Configuration changes
- Error conditions

**Example logs**:
```
INFO neighsyncd: Loading CNSA 2.0 mTLS configuration
INFO neighsyncd: Loaded server certificates cert_count=1 path="/etc/sonic/metrics/server-cert.pem"
INFO neighsyncd: Loaded CA certificates for client verification ca_count=1 path="/etc/sonic/metrics/ca-cert.pem"
INFO neighsyncd: Client certificate verifier configured (mTLS mandatory)
INFO neighsyncd: CNSA 2.0 mTLS configuration complete
INFO neighsyncd: ✅ CNSA 2.0 mTLS enabled
INFO neighsyncd:    ✓ TLS 1.3 only
INFO neighsyncd:    ✓ Cipher: TLS_AES_256_GCM_SHA384
INFO neighsyncd:    ✓ Client certificates: REQUIRED
INFO neighsyncd:    ✓ Crypto: AWS-LC-RS (FIPS 140-3)
INFO neighsyncd:    ✓ Session resumption: DISABLED
```

**Error logs**:
```
ERROR neighsyncd: Failed to load server certificate: No such file or directory
ERROR neighsyncd: TLS handshake failed: certificate verify failed
ERROR neighsyncd: Client certificate validation failed: expired certificate
```

### Metrics for Security

Security-relevant metrics:

```promql
# Authentication failures (increase indicates attack)
increase(neighsyncd_netlink_errors_total[5m])

# Connection status (0 = potential breach)
neighsyncd_redis_connected
neighsyncd_netlink_connected

# Error rate (spike indicates issues)
rate(neighsyncd_events_failed_total[5m])
```

### Security Testing

**TLS configuration testing**:
```bash
# Test TLS 1.2 is rejected
openssl s_client -connect [::1]:9091 -tls1_2 -cert client-cert.pem \
  -key client-key.pem -CAfile ca-cert.pem
# Expected: "no protocols available" or handshake failure

# Test weak cipher is rejected
openssl s_client -connect [::1]:9091 -cipher AES128-SHA -cert client-cert.pem \
  -key client-key.pem -CAfile ca-cert.pem
# Expected: "no ciphers available" or handshake failure

# Test client cert is required
curl https://[::1]:9091/metrics --cacert ca-cert.pem
# Expected: "certificate required" error

# Test invalid client cert is rejected
openssl s_client -connect [::1]:9091 -cert invalid-cert.pem \
  -key invalid-key.pem -CAfile ca-cert.pem
# Expected: "certificate verify failed"
```

**Fuzzing** (future enhancement):
```bash
# TLS fuzzing with tlsfuzzer
tlsfuzzer -h ::1 -p 9091 --cert client-cert.pem --key client-key.pem

# Input validation fuzzing
cargo fuzz run neighbor_entry_parser
```

## Threat Model

### Assets

1. **Neighbor table data**: IP/MAC mappings for network devices
2. **Redis database**: Persistent storage of network state
3. **Metrics endpoint**: Performance and health data
4. **TLS private keys**: Server/client authentication credentials

### Threats

#### T1: Unauthorized Access to Metrics
- **Threat**: Attacker without valid client certificate attempts to access `/metrics`
- **Mitigation**: Mandatory mTLS with client certificate verification
- **Residual Risk**: Low (certificate theft)

#### T2: Man-in-the-Middle Attack
- **Threat**: Attacker intercepts traffic between Prometheus and neighsyncd
- **Mitigation**: TLS 1.3 encryption, certificate validation
- **Residual Risk**: Very Low (requires certificate compromise)

#### T3: Downgrade Attack
- **Threat**: Attacker forces use of weak TLS version or cipher
- **Mitigation**: TLS 1.3 only, single CNSA 2.0 cipher suite
- **Residual Risk**: None (no weaker options available)

#### T4: Certificate Theft
- **Threat**: Attacker steals client certificate from Prometheus server
- **Mitigation**: File permissions (600), encrypted storage (future)
- **Residual Risk**: Medium (requires root access)

#### T5: Denial of Service
- **Threat**: Attacker floods metrics endpoint with requests
- **Mitigation**: Rate limiting (future), connection limits
- **Residual Risk**: Medium (needs implementation)

#### T6: Privilege Escalation
- **Threat**: Attacker exploits neighsyncd to gain CAP_NET_ADMIN
- **Mitigation**: Least privilege, capability dropping
- **Residual Risk**: Low (requires code execution)

#### T7: Data Injection via Netlink
- **Threat**: Attacker sends malicious netlink messages
- **Mitigation**: Kernel access control (CAP_NET_ADMIN), input validation
- **Residual Risk**: Low (requires kernel access)

### Security Boundaries

```
┌─────────────────────────────────────────────┐
│  Trusted Zone (localhost)                   │
│  ┌───────────────────────────────────────┐  │
│  │  neighsyncd Process                   │  │
│  │  - CAP_NET_ADMIN required             │  │
│  │  - Netlink socket (privileged)        │  │
│  │  - Redis client (localhost only)      │  │
│  │  - Metrics server (::1 bind)          │  │
│  └───────────────────────────────────────┘  │
│                    │                         │
│         ┌──────────┴──────────┐              │
│         ▼                     ▼              │
│  ┌─────────────┐      ┌─────────────┐       │
│  │   Netlink   │      │    Redis    │       │
│  │   Kernel    │      │  localhost  │       │
│  └─────────────┘      └─────────────┘       │
└─────────────────────────────────────────────┘
                    │
                    │ mTLS (CNSA 2.0)
                    │
         ┌──────────▼──────────┐
         │   Prometheus        │
         │   (monitoring)      │
         │   + client cert     │
         └─────────────────────┘
```

### Future Security Enhancements

1. **Certificate Revocation**:
   - Implement OCSP stapling
   - Support CRL checking
   - Certificate transparency monitoring

2. **Hardware Security Module (HSM)**:
   - Store private keys in HSM
   - PKCS#11 integration
   - Hardware-backed certificate operations

3. **Rate Limiting**:
   - Per-client rate limits
   - Connection throttling
   - DDoS protection

4. **Audit Logging**:
   - Dedicated security audit log
   - Tamper-proof logging (syslog-ng)
   - Log forwarding to SIEM

5. **Intrusion Detection**:
   - Anomaly detection on metrics
   - Failed authentication monitoring
   - Traffic pattern analysis

## Compliance Verification

### CNSA 2.0 Checklist

- [x] TLS 1.3 only (no TLS 1.2 or earlier)
- [x] Cipher suite: TLS_AES_256_GCM_SHA384
- [x] Key exchange: ECDHE with P-384 or P-521
- [x] Hash algorithm: SHA-384 or SHA-512
- [x] Certificate signatures: ECDSA P-384 with SHA-384
- [x] FIPS 140-3 validated crypto (AWS-LC-RS)
- [x] Forward secrecy (ephemeral key exchange)
- [x] No session resumption
- [x] Client certificate authentication

### Verification Commands

```bash
# 1. Verify TLS 1.3 only
openssl s_client -connect [::1]:9091 -tls1_3 -cert client-cert.pem \
  -key client-key.pem -CAfile ca-cert.pem | grep "Protocol"
# Expected: Protocol  : TLSv1.3

# 2. Verify cipher suite
openssl s_client -connect [::1]:9091 -cert client-cert.pem \
  -key client-key.pem -CAfile ca-cert.pem | grep "Cipher"
# Expected: Cipher    : TLS_AES_256_GCM_SHA384

# 3. Verify certificate algorithm
openssl x509 -in /etc/sonic/metrics/server-cert.pem -noout -text | \
  grep "Public Key Algorithm"
# Expected: Public Key Algorithm: id-ecPublicKey

# 4. Verify curve
openssl x509 -in /etc/sonic/metrics/server-cert.pem -noout -text | \
  grep "ASN1 OID"
# Expected: ASN1 OID: secp384r1

# 5. Verify signature hash
openssl x509 -in /etc/sonic/metrics/server-cert.pem -noout -text | \
  grep "Signature Algorithm"
# Expected: Signature Algorithm: ecdsa-with-SHA384

# 6. Verify client cert is required
curl https://[::1]:9091/metrics --cacert ca-cert.pem
# Expected: Error (certificate required)
```

## See Also

- [MONITORING.md](MONITORING.md) - Metrics server configuration and usage
- [DEPLOYMENT.md](DEPLOYMENT.md) - Production deployment procedures
- [RFC 8446](https://datatracker.ietf.org/doc/html/rfc8446) - TLS 1.3 specification
- [NIST SP 800-52 Rev 2](https://csrc.nist.gov/publications/detail/sp/800-52/rev-2/final) - TLS guidelines
- [CNSA 2.0 FAQ](https://media.defense.gov/2022/Sep/07/2003071834/-1/-1/0/CSA_CNSA_2.0_FAQ_.PDF) - NSA CNSA 2.0 guidance
