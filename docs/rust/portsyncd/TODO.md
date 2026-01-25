# portsyncd TODO Items & Implementation Tasks

**Version**: 1.0
**Date**: January 25, 2026
**Status**: Active Development
**Phase**: Phase 5-8 (Integration & Performance Optimization)

---

## Overview

This document tracks outstanding TODO items and implementation tasks for the portsyncd Rust daemon. All items are organized by phase and priority.

---

## TODO Items Found in Codebase

### 1. Structured Logging Integration (Priority: High)

**File**: `src/main.rs:33`
**Current Status**: Placeholder - returns empty `Ok(())`
**Phase**: Phase 8 Week 6 (Monitoring Integration)

**Description**:
```rust
fn init_logging() -> Result<(), PortsyncError> {
    // TODO: Initialize tracing subscriber for structured logging
    // Will integrate with sonic-audit for NIST 800-53 RFC 5424 compliance
    Ok(())
}
```

**Implementation Details**:
- Initialize `tracing` crate with syslog output
- Integration with `sonic-audit` for compliance logging
- Support RFC 5424 structured logging format
- Attach context to logs (port name, event ID, etc.)
- Configure log levels (DEBUG, INFO, WARN, ERROR)

**Compliance References**:
- NIST 800-53 AU-2 (Audit Events) - see [SECURITY_COMPLIANCE.md](./SECURITY_COMPLIANCE.md#au-2-audit-events)
- RFC 5424 Syslog Protocol
- Application Development STIG APP.6 (Error Handling)

**Related Documentation**:
- [SECURITY_COMPLIANCE.md](./SECURITY_COMPLIANCE.md) - Lines 393-595 (AU-2: Audit Events)
- [LINUX_PERFORMANCE_TUNING.md](./LINUX_PERFORMANCE_TUNING.md) - Performance impact considerations
- [CRATE_ARCHITECTURE.md](./CRATE_ARCHITECTURE.md) - Main event loop structure

**Implementation Plan**:
1. Add `tracing` and `tracing-subscriber` to `Cargo.toml`
2. Configure tracing subscriber with syslog layer
3. Create structured log spans for event processing
4. Add context fields (port_name, event_id, latency_us)
5. Test log output format and performance impact
6. Update systemd service to capture structured logs

**Testing Requirements**:
- Verify structured log format (RFC 5424 compliance)
- Confirm syslog output to systemd journal
- Validate query capabilities (`journalctl`)
- Measure performance impact on event processing
- Test log rotation and retention

**Dependencies**:
- `tracing` crate
- `tracing-subscriber` crate
- `sonic-audit` (pending availability)
- RFC 5424 syslog format knowledge

---

### 2. Real Netlink Event Reception (Priority: Critical)

**File**: `src/main.rs:120`
**Current Status**: Mock - uses 100ms sleep delay
**Phase**: Phase 5 Week 2 (Kernel Netlink Integration)

**Description**:
```rust
// TODO: In production, receive actual netlink events from kernel socket
// For now, simulate a simple delay to prevent busy loop
tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
```

**Implementation Details**:
- Replace 100ms sleep with blocking netlink socket read
- Implement epoll/poll for event-driven processing
- Parse RTM_NEWLINK and RTM_DELLINK messages
- Extract port name, flags, MTU from netlink attributes
- Handle message overflow and drops

**Compliance References**:
- NIST 800-53 SC-7 (Boundary Protection) - see [SECURITY_COMPLIANCE.md](./SECURITY_COMPLIANCE.md#sc-7-boundary-protection)
- NIST 800-53 SI-4 (System Monitoring)
- Linux Netlink Protocol (man7.org/linux/man-pages/man7/netlink.7.html)

**Related Documentation**:
- [SECURITY_COMPLIANCE.md](./SECURITY_COMPLIANCE.md) - Lines 24-62 (SC-7: Boundary Protection)
- [CRATE_ARCHITECTURE.md](./CRATE_ARCHITECTURE.md) - Lines 161-201 (Port Synchronization Flow)
- [LINUX_PERFORMANCE_TUNING.md](./LINUX_PERFORMANCE_TUNING.md) - Lines 127-141 (I/O Multiplexing)
- [PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md](./PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md) - Weeks 1-2

**Implementation Plan** (from Phase 5 Week 2):
1. **Task 2.1**: Implement epoll-based blocking event reception (4 days effort)
   - Create `receive_events_blocking()` function
   - Subscribe only to RTNLGRP_LINK multicast group
   - Handle poll timeout and errors
   - Test with blocking wait and timeout behavior

2. **Task 2.2**: Integrate epoll into main event loop (3 days effort)
   - Replace sleep loop with epoll blocking call
   - Handle signal interruption (SIGTERM, SIGINT)
   - Integrate with metrics collection
   - Benchmark CPU usage target: <10%

3. **Task 2.3**: Implement batch event processing (3 days effort)
   - Process multiple events from single epoll call
   - Use Redis PIPE for batch writes
   - Handle errors in batch operations
   - Target throughput: >20K events/second

**Testing Requirements**:
- Verify RTNLGRP_LINK subscription (only port change events)
- Test blocking behavior with timeout
- Validate message parsing accuracy
- Confirm overflow detection (MSG_TRUNC flag)
- Stress test with high event rates (10K+ eps)
- Performance regression tests

**Dependencies**:
- `nix` crate (for epoll, socket operations)
- `netlink-packet-route` crate (if needed for parsing)
- Kernel netlink support

**Performance Targets**:
- Event processing latency: <100μs (P50)
- Throughput: >20K events/second
- CPU usage: <10% sustained
- No message loss under 15K eps burst

---

## Implementation Phase Timeline

### Phase 5: Real Integration (Weeks 1-3)

**Week 1**: Netlink Socket Optimization
- [ ] Implement socket buffer tuning (SO_RCVBUF=16MB, SO_SNDBUF=2MB)
- [ ] Add MSG_TRUNC overflow handling
- [ ] Subscribe to RTNLGRP_LINK multicast group
- Reference: [PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md](./PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md) - Week 1

**Week 2**: I/O Multiplexing (Related to TODO #2)
- [ ] Implement epoll-based event reception
- [ ] Integrate epoll into main event loop
- [ ] Implement batch event processing
- Reference: [PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md](./PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md) - Week 2

**Week 3**: Memory Optimization
- [ ] Implement buffer pool (zero-allocation)
- [ ] Implement memory locking (mlock)
- [ ] Configure CPU affinity
- Reference: [PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md](./PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md) - Week 3

### Phase 8: Performance Tuning & Finalization (Weeks 1-8)

**Week 6**: Monitoring Integration (Related to TODO #1)
- [ ] Implement kernel metrics collection
- [ ] Create Prometheus dashboard
- [ ] Implement health check endpoints
- [ ] Initialize structured logging
- Reference: [PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md](./PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md) - Week 6

**Week 8**: Documentation & Finalization
- [ ] Complete performance tuning documentation
- [ ] Create deployment guide
- [ ] Final testing and sign-off
- Reference: [PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md](./PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md) - Week 8

---

## Related Tasks & Documentation

### Security & Compliance
- **NIST 800-53 Rev5 Controls**: [SECURITY_COMPLIANCE.md](./SECURITY_COMPLIANCE.md)
  - SC-7: Boundary Protection (netlink multicast filtering)
  - SI-4: System Monitoring (metrics & alerting)
  - AU-2: Audit Events (structured logging)
  - SC-24: Fail-Secure Restart
  - IA-2: Authentication
  - AC-3: Access Control

- **DISA Application Development STIGs**: [SECURITY_COMPLIANCE.md](./SECURITY_COMPLIANCE.md)
  - APP.1: Code Quality
  - APP.2: Configuration Management
  - APP.3: Secure Communication
  - APP.4: Cryptography
  - APP.5: Session Management
  - APP.6: Error Handling

### Architecture & Design
- **Component Interactions**: [CRATE_ARCHITECTURE.md](./CRATE_ARCHITECTURE.md)
  - Module dependency graph (lines 23-104)
  - Port synchronization flow (lines 161-201)
  - Main event loop orchestration (lines 110-157)
  - Performance characteristics (lines 598-628)

### Performance Optimization
- **Linux Kernel Tuning**: [LINUX_PERFORMANCE_TUNING.md](./LINUX_PERFORMANCE_TUNING.md)
  - Netlink buffer optimization (lines 27-125)
  - I/O multiplexing with epoll (lines 127-141)
  - Memory locking and CPU affinity (lines 143-300)
  - Network stack tuning (lines 302-450)
  - Troubleshooting guide (lines 782-843)

- **Implementation Plan**: [PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md](./PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md)
  - Week-by-week breakdown (8 weeks total)
  - Task estimation (effort in days)
  - Success metrics for each phase
  - Risk mitigation strategies
  - Resource requirements

---

## Dependency Map

```
TODO #2 (Netlink Events)
├─ netlink_socket.rs (parsing)
├─ port_sync.rs (event handling)
├─ metrics.rs (latency measurement)
└─ main.rs (event loop)
    └─ LINUX_PERFORMANCE_TUNING.md (kernel tuning)
    └─ PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md (Week 2)

TODO #1 (Structured Logging)
├─ init_logging() in main.rs
├─ tracing crate integration
├─ systemd journal output
└─ SECURITY_COMPLIANCE.md (AU-2: Audit Events)
    └─ PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md (Week 6)
```

---

## Current Blockers

### TODO #2: Netlink Events (Blocking Implementation)
- **Status**: Waiting for Phase 5 Week 2 scheduling
- **Blocker**: Requires kernel netlink socket implementation
- **Workaround**: Current mock uses 100ms sleep (acceptable for testing)
- **Impact**: Limits event throughput to ~10 eps, prevents real performance validation

### TODO #1: Structured Logging (Non-blocking)
- **Status**: Acceptable to defer until Phase 8 Week 6
- **Blocker**: Pending `sonic-audit` integration (may not be available)
- **Workaround**: Can use direct `tracing` crate without sonic-audit
- **Impact**: Limited to stderr/eprintln, no structured journalctl output

---

## Success Criteria

### TODO #2 Completion
- ✅ RTM_NEWLINK/DELLINK events parsed correctly
- ✅ Event latency <100μs (P50)
- ✅ Throughput >20K events/second
- ✅ No message loss under 15K eps
- ✅ All 451 tests passing (100% pass rate)
- ✅ Zero unsafe code
- ✅ Zero compiler warnings

### TODO #1 Completion
- ✅ Structured logs to systemd journal
- ✅ RFC 5424 syslog format
- ✅ Queryable via `journalctl`
- ✅ NIST AU-2 compliance verified
- ✅ <5% performance impact on event processing
- ✅ All 451 tests passing

---

## References

### Code Files
- `src/main.rs` - Main daemon entry point (TODO items here)
- `src/netlink_socket.rs` - Kernel netlink interface
- `src/port_sync.rs` - Port state synchronization
- `src/metrics.rs` - Metrics collection
- `src/production_features.rs` - Health monitoring

### Documentation Files
1. [SECURITY_COMPLIANCE.md](./SECURITY_COMPLIANCE.md) - NIST & STIG compliance
2. [CRATE_ARCHITECTURE.md](./CRATE_ARCHITECTURE.md) - Component design
3. [LINUX_PERFORMANCE_TUNING.md](./LINUX_PERFORMANCE_TUNING.md) - Kernel optimization
4. [PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md](./PERFORMANCE_TUNING_IMPLEMENTATION_PLAN.md) - Phase 8 roadmap

### External References
- [man7.org netlink(7)](https://man7.org/linux/man-pages/man7/netlink.7.html)
- [man7.org epoll(7)](https://man7.org/linux/man-pages/man7/epoll.7.html)
- [NIST 800-53 Rev5](https://csrc.nist.gov/publications/detail/sp/800-53/rev-5/final)
- [DISA Application Development STIGs](https://public.cyber.mil/stigs/)

---

**Last Updated**: January 25, 2026
**Next Review**: Upon Phase 5 Week 2 completion
**Maintained By**: portsyncd Development Team
