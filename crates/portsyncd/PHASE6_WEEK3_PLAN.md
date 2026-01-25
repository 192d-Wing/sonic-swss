# Phase 6 Week 3: Warm Restart Enhancements - IMPLEMENTATION PLAN

**Date**: 2026-01-24
**Previous Phase Status**: Phase 6 Week 2 COMPLETE (166 tests passing, 41 new tests)
**Current Phase**: Week 3 - Timeout Fallback & State Cleanup
**Target**: 50+ new tests, production-hardening, performance optimization

---

## Overview

Phase 6 Week 3 builds on the solid foundation of Week 2 by adding three critical enhancements:

1. **Timeout-Based EOIU Fallback** - Prevent daemon stall if EOIU signal never arrives
2. **State File Lifecycle Management** - Cleanup, rotation, and corruption recovery
3. **Warm Restart Metrics & Observability** - Track timing, success rate, state transitions

These enhancements ensure the warm restart system is resilient, observable, and production-ready.

---

## Phase 6 Week 2 Summary (Foundation)

**Completed Deliverables**:
- ✅ WarmRestartManager - 10 unit tests
- ✅ EoiuDetector - 8 unit tests
- ✅ LinkSync integration - 5 new tests
- ✅ NetlinkSocket integration - 4 new tests
- ✅ Integration tests - 14 comprehensive tests
- **Total**: 166 tests passing, 41 new tests

**Current Capabilities**:
- Cold start vs warm start detection
- Port state persistence (JSON)
- APP_DB write gating during initial sync
- EOIU signal detection (ifi_change == 0)
- Fail-secure error handling

---

## Phase 6 Week 3 Implementation Plan

### Week 3 Task Breakdown

#### Task 1: Timeout-Based EOIU Fallback (Tasks 1-3)

**Goal**: Prevent daemon stall if EOIU signal never arrives

**Problem**:
- Current implementation waits indefinitely for EOIU signal
- If kernel sends no EOIU, warm restart state remains locked
- APP_DB updates disabled forever → port state never updates

**Solution**:
- Add timeout timer (default 10 seconds)
- Auto-complete initial sync if timeout expires
- Log timeout event for debugging
- Make timeout configurable via environment variable

**Implementation** (Task 1-3):

```rust
// src/warm_restart.rs enhancements
pub struct WarmRestartManager {
    state: WarmRestartState,
    // ... existing fields ...
    initial_sync_start: Option<Instant>,     // NEW: Track when sync started
    initial_sync_timeout_secs: u64,          // NEW: Configurable timeout
}

impl WarmRestartManager {
    pub fn check_initial_sync_timeout(&mut self) -> Result<()> {
        if self.state != WarmRestartState::InitialSyncInProgress {
            return Ok(());
        }

        if let Some(start_time) = self.initial_sync_start {
            let elapsed = start_time.elapsed().as_secs();
            if elapsed >= self.initial_sync_timeout_secs {
                eprintln!(
                    "portsyncd: EOIU timeout after {} seconds, completing initial sync",
                    elapsed
                );
                self.complete_initial_sync();
            }
        }

        Ok(())
    }

    pub fn set_initial_sync_timeout(&mut self, secs: u64) {
        self.initial_sync_timeout_secs = secs;
    }
}
```

**Files Modified**:
- `src/warm_restart.rs` (+60 lines)

**Tests** (8 tests):
- Timeout not reached (normal EOIU case)
- Timeout reached → auto-complete
- Configurable timeout
- Elapsed time calculation
- State transition on timeout
- Multiple timeout checks
- Timeout with no initial sync
- Edge case: zero timeout

#### Task 2: State File Lifecycle Management (Tasks 4-6)

**Goal**: Manage port_state.json lifecycle - cleanup, rotation, recovery

**Problem**:
- Stale state files can cause issues
- No backup/rotation mechanism
- Corruption detected but not cleaned up automatically

**Solutions**:

**Task 4: Auto-Cleanup of Old State Files**

```rust
// src/warm_restart.rs new function
pub fn cleanup_stale_state_files(&self, max_age_days: u32) -> Result<()> {
    // Check if state file exists and is too old
    // If age > max_age_days (default 7), delete it
    // Next restart will be cold start (safe)

    if !self.state_file_path.exists() {
        return Ok(());
    }

    let metadata = fs::metadata(&self.state_file_path)?;
    let modified = metadata.modified()?;
    let age = std::time::SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default()
        .as_secs_f64 / 86400.0; // Convert to days

    if age > max_age_days as f64 {
        eprintln!(
            "portsyncd: Deleting stale state file (age: {:.1} days)",
            age
        );
        fs::remove_file(&self.state_file_path)?;
    }

    Ok(())
}
```

**Task 5: State File Rotation**

```rust
// src/warm_restart.rs new function
pub fn rotate_state_file(&self) -> Result<()> {
    // Before saving new state:
    // 1. If current file exists, move to .backup
    // 2. Save new state to current
    // Enables recovery if new state is corrupted

    if self.state_file_path.exists() {
        let backup_path = format!("{}.backup", self.state_file_path.display());
        fs::rename(&self.state_file_path, backup_path)?;
    }

    self.save_state()?;
    Ok(())
}
```

**Task 6: Corruption Recovery**

```rust
// src/warm_restart.rs enhanced
pub fn load_state_with_recovery(&mut self) -> Result<()> {
    // Try to load current state file
    match self.load_state() {
        Ok(()) => Ok(()),
        Err(_) => {
            // Current file corrupted, try backup
            let backup_path = format!("{}.backup", self.state_file_path.display());
            if Path::new(&backup_path).exists() {
                eprintln!("portsyncd: Current state corrupted, recovering from backup");
                fs::rename(backup_path, &self.state_file_path)?;
                self.load_state()?;
                Ok(())
            } else {
                // No backup available, cold start
                eprintln!("portsyncd: State recovery failed, falling back to cold start");
                Ok(())
            }
        }
    }
}
```

**Files Modified**:
- `src/warm_restart.rs` (+80 lines)

**Tests** (12 tests):
- Cleanup stale files (7+ days old)
- Keep recent files
- Rotation creates backup
- Rotation saves new state
- Recovery from corrupted current file
- Recovery from corrupted backup
- Recovery when no backup exists
- Max age configurable
- Cleanup during initialization
- Backup file cleanup
- Multiple rotation cycles
- Edge case: missing backup directory

#### Task 3: Metrics & Observability (Tasks 7-9)

**Goal**: Track warm restart metrics for monitoring and debugging

**Task 7: Create WarmRestartMetrics struct**

```rust
// src/warm_restart.rs new struct
pub struct WarmRestartMetrics {
    cold_starts: u64,
    warm_starts: u64,
    eoiu_timeouts: u64,
    eoiu_received: u64,
    state_save_failures: u64,
    state_load_failures: u64,
    avg_initial_sync_time_ms: u64,
}

impl WarmRestartMetrics {
    pub fn new() -> Self {
        Self {
            cold_starts: 0,
            warm_starts: 0,
            eoiu_timeouts: 0,
            eoiu_received: 0,
            state_save_failures: 0,
            state_load_failures: 0,
            avg_initial_sync_time_ms: 0,
        }
    }

    pub fn record_cold_start(&mut self) { self.cold_starts += 1; }
    pub fn record_warm_start(&mut self) { self.warm_starts += 1; }
    pub fn record_eoiu_timeout(&mut self) { self.eoiu_timeouts += 1; }
    pub fn record_eoiu_received(&mut self) { self.eoiu_received += 1; }

    pub fn success_rate(&self) -> f64 {
        let total = self.cold_starts + self.warm_starts;
        if total == 0 { 0.0 } else { 100.0 * self.warm_starts as f64 / total as f64 }
    }
}
```

**Task 8: Integrate metrics into WarmRestartManager**

```rust
impl WarmRestartManager {
    metrics: WarmRestartMetrics,  // NEW field

    pub fn metrics(&self) -> &WarmRestartMetrics {
        &self.metrics
    }

    pub fn initialize(&mut self) -> Result<()> {
        if self.should_warm_start() {
            self.metrics.record_warm_start();
        } else {
            self.metrics.record_cold_start();
        }
        // ... rest of initialization ...
    }

    pub fn complete_initial_sync(&mut self) {
        if self.state == WarmRestartState::InitialSyncInProgress {
            // Check if EOIU was received or timeout occurred
            let elapsed = self.initial_sync_start.map(|t| t.elapsed().as_millis() as u64);
            self.metrics.avg_initial_sync_time_ms = elapsed.unwrap_or(0);

            if elapsed.unwrap_or(0) >= self.initial_sync_timeout_secs * 1000 {
                self.metrics.record_eoiu_timeout();
            } else {
                self.metrics.record_eoiu_received();
            }

            self.state = WarmRestartState::InitialSyncComplete;
        }
    }
}
```

**Task 9: Expose metrics via public API**

```rust
// In LinkSync or main.rs
pub fn warm_restart_metrics(&self) -> Option<&WarmRestartMetrics> {
    self.warm_restart.as_ref().map(|mgr| mgr.metrics())
}

// Usage in main:
if let Some(metrics) = link_sync.warm_restart_metrics() {
    eprintln!(
        "portsyncd: Warm restart success rate: {:.1}%",
        metrics.success_rate()
    );
}
```

**Files Modified**:
- `src/warm_restart.rs` (+100 lines)
- `src/port_sync.rs` (+5 lines)

**Tests** (10 tests):
- Metrics creation
- Cold/warm start recording
- EOIU timeout recording
- EOIU received recording
- Success rate calculation
- Initial sync timing
- Failure counting
- Metrics persistence
- Metrics reset
- Concurrent metric updates

---

## Integration Test Suite

### New Integration Tests (20 tests)

**File**: `tests/warm_restart_week3_integration.rs` (500+ lines)

**Test Categories**:

**Timeout Fallback** (6 tests):
1. Timeout not reached → EOIU completes normally
2. Timeout reached → Auto-complete initial sync
3. Timeout with no EOIU signal → Safe transition
4. Multiple timeout checks → Prevents double-completion
5. Zero timeout → Immediate completion
6. Very long timeout → Waits correctly

**State File Lifecycle** (8 tests):
1. Cleanup stale files (7+ days old)
2. Keep recent files (< 7 days)
3. Rotation creates backup
4. Rotation preserves port state
5. Recovery from corrupted current file
6. Recovery from corrupted backup
7. Recovery chain: current → backup → cold start
8. Backup cleanup after successful recovery

**Metrics & Observability** (6 tests):
1. Cold start recorded in metrics
2. Warm start recorded in metrics
3. EOIU timeout recorded
4. EOIU received recorded
5. Success rate calculation
6. Initial sync timing measurement

---

## Implementation Schedule

### Timeline (Week 3)

**Day 1-2: Timeout Fallback**
- Implement timeout detection (Task 1)
- Add configurable timeout (Task 2)
- Auto-complete on timeout (Task 3)
- 8 unit tests + 2 integration tests = 10 tests

**Day 3-4: State File Lifecycle**
- Cleanup stale files (Task 4)
- Rotation mechanism (Task 5)
- Corruption recovery (Task 6)
- 12 unit tests + 3 integration tests = 15 tests

**Day 5: Metrics & Observability**
- Create WarmRestartMetrics (Task 7)
- Integrate into manager (Task 8)
- Expose via API (Task 9)
- 10 unit tests + 4 integration tests = 14 tests

**Daily**: Running test suite, documentation

---

## Testing Strategy

### Unit Tests (30 tests)

```
warm_restart.rs enhancements:
  Timeout Detection:        5 tests
  Timeout Configuration:    3 tests
  Stale File Cleanup:       4 tests
  State Rotation:           3 tests
  Corruption Recovery:      4 tests
  Metrics:                  4 tests
  Integration:              0 tests
  ──────────────────────────
  TOTAL:                   23 tests

port_sync.rs enhancements:
  Metrics Exposure:         2 tests
  Timeout Integration:      2 tests
  ──────────────────────────
  TOTAL:                    4 tests

Other modules:
  Existing tests:          (unchanged)
  ──────────────────────────
  TOTAL:                    3 tests
```

### Integration Tests (20 tests)

```
warm_restart_week3_integration.rs:
  Timeout Fallback:         6 tests
  State File Lifecycle:     8 tests
  Metrics & Observability:  6 tests
  ──────────────────────────
  TOTAL:                   20 tests
```

### Target Results

```
Unit Tests:           180+ (currently 152 + 30 new)
Integration Tests:    34+ (currently 14 + 20 new)
Total:                214+ tests
Success Rate:         100% ✅
```

---

## Architecture Updates

### Timeout Fallback Flow

```
WarmStartState:InitialSyncInProgress
  │
  ├─ [Timer started when state entered]
  │
  ├─ [Receive netlink events]
  │  ├─ RTM_NEWLINK: Ethernet0 (normal)
  │  ├─ RTM_NEWLINK: Ethernet4 (normal)
  │  └─ [No EOIU signal received]
  │
  ├─ [Every event: check timeout]
  │  └─ elapsed >= initial_sync_timeout_secs?
  │
  ├─ YES: Timeout reached
  │  └─ AUTO-COMPLETE
  │      ├─ Transition: InitialSyncInProgress → InitialSyncComplete
  │      ├─ Enable APP_DB updates
  │      └─ Record timeout metric
  │
  └─ NO: Keep waiting
```

### State File Lifecycle

```
[On daemon startup]
  │
  ├─ Check state file age
  │  └─ Age > 7 days? → DELETE (cleanup_stale_state_files)
  │
  ├─ Load state
  │  └─ Success? → Use it
  │  └─ Fail? → Try backup (load_state_with_recovery)
  │
  ├─ Recover from backup
  │  └─ Backup exists? → Restore and use
  │  └─ Backup missing? → Cold start (safe)
  │
[During runtime]
  │
  └─ Periodic: save_state()
     ├─ Rotate files (current → backup)
     └─ Save new state atomically
```

### Metrics Tracking

```
WarmRestartManager
  │
  ├─ on initialize:
  │  └─ Record cold_start or warm_start
  │
  ├─ on initial_sync_start:
  │  └─ Record start_time
  │
  ├─ on EOIU received:
  │  └─ Record elapsed_time
  │  └─ Record eoiu_received
  │
  ├─ on timeout:
  │  └─ Record elapsed_time
  │  └─ Record eoiu_timeout
  │
  └─ Success rate:
     = warm_starts / (warm_starts + cold_starts) * 100%
```

---

## Configuration

### Environment Variables

```bash
# Timeout control (Task 1-2)
PORTSYNCD_EOIU_TIMEOUT_SECS=10        # Default: 10 seconds

# State file control (Task 4-5)
PORTSYNCD_STATE_MAX_AGE_DAYS=7        # Default: 7 days
PORTSYNCD_STATE_ROTATION=true         # Default: true
PORTSYNCD_STATE_RECOVERY=true         # Default: true

# Metrics control (Task 7-9)
PORTSYNCD_METRICS_ENABLED=true        # Default: true
```

### Code Integration

```rust
// In main.rs or LinkSync initialization

// Configure timeout
let mut link_sync = LinkSync::with_warm_restart(state_file_path)?;
if let Ok(timeout) = std::env::var("PORTSYNCD_EOIU_TIMEOUT_SECS") {
    if let Ok(secs) = timeout.parse::<u64>() {
        link_sync.set_initial_sync_timeout(secs);
    }
}

// Run periodic checks
loop {
    link_sync.check_initial_sync_timeout()?;

    if let Some(metrics) = link_sync.warm_restart_metrics() {
        eprintln!(
            "Warm restart success rate: {:.1}%",
            metrics.success_rate()
        );
    }
}
```

---

## Success Criteria

### Code Quality
- ✅ 50+ new tests (exceeds 40 target)
- ✅ Zero compiler warnings (new code)
- ✅ Zero unsafe code (new code)
- ✅ Full documentation

### Functionality
- ✅ Timeout fallback working (auto-complete on timeout)
- ✅ State cleanup functional (stale files removed)
- ✅ File rotation working (backup created)
- ✅ Recovery functional (backup restores on corruption)
- ✅ Metrics accurate (timing, counts recorded)

### Resilience
- ✅ No daemon stall (timeout prevents infinite wait)
- ✅ Handles corruption gracefully (backup recovery)
- ✅ Cleanup prevents issues (old state cleanup)
- ✅ Observable (metrics for monitoring)

### Performance
- ✅ Timeout check <1ms (on every event)
- ✅ Cleanup <5ms (startup only)
- ✅ Metrics overhead <0.1% (atomic updates)

---

## Risk Mitigation

| Risk | Mitigation |
|------|-----------|
| EOIU never arrives | Timeout fallback (10s default) |
| Timeout too short | Configurable via env var |
| Timeout too long | Daemon stalls on timeout | Make configurable |
| Stale state file used | Auto-cleanup (7 day max age) |
| Backup corrupted too | Fallback to cold start (safe) |
| Metrics overhead | Atomic operations only (<0.1%) |
| Rotation fails | Continue with current state (graceful) |

---

## Deliverables Checklist

### Code
- [ ] Timeout fallback in WarmRestartManager
- [ ] State cleanup (old files)
- [ ] State rotation (backup mechanism)
- [ ] Corruption recovery (backup chain)
- [ ] WarmRestartMetrics struct
- [ ] Metrics integration
- [ ] Metrics API exposure

### Tests
- [ ] 8 timeout unit tests
- [ ] 12 state lifecycle unit tests
- [ ] 10 metrics unit tests
- [ ] 20 integration tests
- [ ] All passing (100%)

### Documentation
- [ ] API documentation (code comments)
- [ ] Configuration guide
- [ ] Deployment guide
- [ ] Troubleshooting guide
- [ ] Migration guide (Week 2 → Week 3)

### Quality
- [ ] Zero warnings (new code)
- [ ] Zero unsafe code (new code)
- [ ] Type-safe APIs
- [ ] Error handling complete

---

## Post-Week 3: Phase 6 Week 4+

**Potential Enhancements**:
- Distributed state coordination (HA scenarios)
- Multi-daemon warm restart synchronization
- Persistent metrics (write to file/database)
- Prometheus metrics for warm restart
- Chaos testing (failure injection)
- Performance benchmarking vs C++ portsyncd

---

## Summary

Phase 6 Week 3 adds production-hardening features to the warm restart system:

✅ **Timeout Fallback** - Prevent daemon stall if EOIU never arrives
✅ **State Lifecycle** - Cleanup old files, rotate with backup, recover from corruption
✅ **Metrics** - Track success rate, timing, failure modes
✅ **50+ Tests** - Exceeds quality target, 100% pass rate
✅ **Production Ready** - Resilient, observable, maintainable

The warm restart system moves from "functional" to "production-hardened" with these enhancements.

---

**Plan Status**: Ready for implementation
**Target Tests**: 50+ new tests (214+ total)
**Timeline**: 5 days
**Next Phase**: Phase 6 Week 4 (distributed coordination)

