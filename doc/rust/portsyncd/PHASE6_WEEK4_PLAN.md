# Phase 6 Week 4 - Advanced Features & Metrics Export

## Overview

Build upon Phase 6 Week 3's foundation to implement persistent metrics storage,
configurable retention policies, and Prometheus-compatible metrics export. This
adds production-grade observability and metrics persistence across daemon
restarts.

## Architecture

```text
Phase 6 Week 4 Components:

WarmRestartManager
  ├─ In-Memory Metrics (current)
  │   └─ WarmRestartMetrics struct
  │
  └─ Persistent Metrics (NEW)
      ├─ Save to JSON file
      ├─ Load on startup
      └─ Periodic auto-save

MetricsExporter (NEW)
  ├─ Prometheus format export
  ├─ HTTP endpoint (/metrics)
  └─ Gauge, Counter, Histogram types

ConfigFile (ENHANCED)
  ├─ Metrics retention policy
  ├─ Metrics save interval
  ├─ Export format selection
  └─ Persistent storage location

AnalyticsAggregator (NEW)
  ├─ Min/Max/Avg calculations
  ├─ Percentile calculations
  ├─ Recovery success rate
  ├─ EOIU latency analysis
  └─ Trend detection
```

## Tasks

### Task 1: Persistent Metrics Storage

**Goal**: Save and load metrics from JSON file to survive daemon restarts

**Implementation**:

- File: `src/warm_restart.rs`
- Add methods to WarmRestartManager:
  - `save_metrics(path: &Path) -> Result<()>` - Serialize metrics to JSON
  - `load_metrics(path: &Path) -> Result<()>` - Deserialize metrics from JSON
  - `metrics_file_path() -> PathBuf` - Get metrics file path from config
- Auto-save on significant events (every N events or periodic)
- Load metrics on WarmRestartManager initialization

**Tests**: 8 tests

- Save and load metrics round-trip
- Metrics persistence across restarts
- Corrupt metrics file recovery
- Metrics accumulation across multiple cycles
- Edge cases (empty metrics, large numbers)
- Atomic writes (no partial/corrupt files)
- Path handling and directory creation

**Expected Lines**: ~150 LOC

### Task 2: Configurable Retention Policies

**Goal**: Allow operators to configure metrics retention via config file

**Implementation**:

- File: `src/config_file.rs` (or new `src/metrics_config.rs`)
- Add MetricsConfig struct:

  ```rust
  pub struct MetricsConfig {
      pub enabled: bool,
      pub save_interval_secs: u64,  // Auto-save interval
      pub retention_days: u64,        // Keep metrics for N days
      pub max_file_size_mb: u64,      // Rotate file if too large
      pub export_format: ExportFormat, // prometheus, json, both
      pub storage_path: PathBuf,
  }
  ```

- Parse from config file: `/etc/sonic/portsyncd.conf`
- Validation: ensure sane values
- Default values for all fields

**Tests**: 6 tests

- Config file parsing
- Default values applied
- Validation of constraints
- Invalid config handling
- Path expansion and creation
- Type conversions

**Expected Lines**: ~120 LOC

### Task 3: Prometheus Metrics Export

**Goal**: Export metrics in Prometheus format for scraping

**Implementation**:

- File: `src/metrics_exporter.rs` (new)
- PrometheusExporter struct:

  ```rust
  pub struct PrometheusExporter {
      metrics: &'static WarmRestartMetrics,
  }
  ```

- Methods:
  - `new(metrics: &WarmRestartMetrics) -> Self`
  - `export(&self) -> String` - Return Prometheus format
  - `export_json(&self) -> Result<String>` - JSON format
- Export format:

  ```text
  # HELP portsyncd_warm_restarts Total warm restart events
  # TYPE portsyncd_warm_restarts counter
  portsyncd_warm_restarts 5

  # HELP portsyncd_cold_starts Total cold start events
  # TYPE portsyncd_cold_starts counter
  portsyncd_cold_starts 1

  # HELP portsyncd_eoiu_detected Total EOIU signals detected
  # TYPE portsyncd_eoiu_detected counter
  portsyncd_eoiu_detected 4

  # HELP portsyncd_eoiu_timeouts Total EOIU timeouts (auto-complete)
  # TYPE portsyncd_eoiu_timeouts counter
  portsyncd_eoiu_timeouts 1

  # HELP portsyncd_state_recoveries Total successful state recoveries
  # TYPE portsyncd_state_recoveries counter
  portsyncd_state_recoveries 2

  # HELP portsyncd_corruptions_detected Total corruption events
  # TYPE portsyncd_corruptions_detected counter
  portsyncd_corruptions_detected 1

  # HELP portsyncd_sync_duration_seconds Initial sync duration (seconds)
  # TYPE portsyncd_sync_duration_seconds histogram
  portsyncd_sync_duration_seconds_sum 125.5
  portsyncd_sync_duration_seconds_count 2
  portsyncd_sync_duration_seconds_bucket{le="10"} 0
  portsyncd_sync_duration_seconds_bucket{le="50"} 1
  portsyncd_sync_duration_seconds_bucket{le="100"} 2
  portsyncd_sync_duration_seconds_bucket{le="+Inf"} 2
  ```

- Integration with existing metrics_server.rs
- HTTP endpoint: GET /metrics -> Prometheus format

**Tests**: 10 tests

- Format validation
- All metric types exported
- Timestamp handling
- Large numbers formatting
- Special characters in labels
- Empty metrics export
- JSON export format
- Endpoint response codes
- Content-Type headers
- Concurrent access

**Expected Lines**: ~200 LOC

### Task 4: Metrics Analytics & Aggregation

**Goal**: Calculate insights from collected metrics

**Implementation**:

- File: `src/warm_restart.rs` (extend WarmRestartMetrics)
- Add methods to WarmRestartMetrics:

  ```rust
  pub fn recovery_success_rate(&self) -> f64
  pub fn corruption_recovery_rate(&self) -> f64
  pub fn eoiu_timeout_rate(&self) -> f64
  pub fn avg_events_per_restart(&self) -> f64
  pub fn uptime_estimate(&self) -> Duration
  pub fn last_healthy_restart(&self) -> Option<SystemTime>
  pub fn is_system_healthy(&self) -> bool
  ```

- AnalyticsAggregator struct:
  - Percentile calculations (p50, p95, p99)
  - Trend detection (is recovery rate increasing?)
  - Anomaly detection (unusual event patterns)

**Tests**: 8 tests

- Recovery rate calculations
- Edge cases (zero denominator)
- Percentile calculations
- Trend detection
- Health assessment
- Uptime estimation
- Rate limiting detection
- Anomaly thresholds

**Expected Lines**: ~180 LOC

### Task 5: Deployment & Configuration Files

**Goal**: Production-ready systemd service and config template

**Deliverables**:

- `portsyncd.service` - Systemd unit file
  - Type: notify
  - WatchdogSec: 30s
  - RestartSec: 5
  - Restart: on-failure
  - SuccessExitStatus: 0 143

- `portsyncd.conf.example` - Configuration template

  ```toml
  [daemon]
  enabled = true
  log_level = "info"

  [database]
  redis_host = "127.0.0.1"
  redis_port = 6379

  [warm_restart]
  enabled = true
  eoiu_timeout_secs = 10
  state_file = "/var/lib/sonic/portsyncd/port_state.json"

  [metrics]
  enabled = true
  save_interval_secs = 300
  retention_days = 30
  max_file_size_mb = 100
  export_format = "prometheus"
  storage_path = "/var/lib/sonic/portsyncd/metrics"

  [health]
  max_stall_seconds = 10
  max_failure_rate_percent = 5.0
  ```

- `README_DEPLOYMENT.md` - Deployment guide

**Tests**: 4 tests

- Config file parsing
- Default values
- Path validation
- Service file syntax

### Task 6: Integration Tests for Metrics Export

**Goal**: End-to-end metrics persistence and export testing

**Tests** (12 total):

- Metrics persist across daemon restarts
- Prometheus export format valid
- Metrics accumulate correctly
- Config file controls behavior
- Analytics calculations accurate
- Health status reflects metrics
- Corrupt metrics recovery
- Metrics rotation on size threshold
- Retention cleanup removes old files
- Concurrent metrics access
- Export endpoint responds correctly
- Performance: export <100ms

**Expected test file**: `tests/metrics_export_integration.rs` (~800 LOC)

## Implementation Sequence

### Week 4 Timeline

**Day 1-2**: Persistent Metrics Storage (Task 1)

- Implement save/load methods
- Add metrics file handling
- Write 8 unit tests
- Verify round-trip serialization

**Day 2-3**: Configuration System (Task 2)

- Create MetricsConfig struct
- Parse config file
- Write 6 unit tests
- Validate all paths

**Day 3-4**: Prometheus Export (Task 3)

- Create PrometheusExporter
- Implement format generation
- Write 10 unit tests
- Add HTTP endpoint integration

**Day 4-5**: Analytics (Task 4)

- Add calculation methods
- Implement percentile logic
- Write 8 unit tests
- Test anomaly detection

**Day 5-6**: Deployment Files (Task 5)

- Create systemd unit file
- Create config template
- Write deployment guide
- 4 integration tests

**Day 6-7**: Integration Tests (Task 6)

- Full end-to-end scenarios
- 12 comprehensive tests
- Performance validation
- Stress testing

## Success Criteria

✅ **Persistent Storage**:

- Metrics survive daemon restart
- No data loss on corruption
- Automatic recovery from backups

✅ **Configuration**:

- All metrics config options work
- Defaults apply correctly
- Validation prevents invalid states

✅ **Prometheus Export**:

- Valid Prometheus format
- All metrics included
- Performance <100ms export time

✅ **Analytics**:

- Accurate calculations
- Trend detection working
- Health assessment reliable

✅ **Testing**:

- 50+ new unit tests
- 12 integration tests
- 100% test pass rate
- Stress test (1000 events/sec)

✅ **Quality**:

- 0 warnings, 0 unsafe code
- All clippy suggestions addressed
- Comprehensive documentation

## Files to Create/Modify

### New Files

- `src/metrics_exporter.rs` - Prometheus export logic
- `portsyncd.service` - Systemd unit file
- `portsyncd.conf.example` - Config template
- `README_DEPLOYMENT.md` - Deployment guide
- `tests/metrics_export_integration.rs` - Integration tests

### Modified Files

- `src/warm_restart.rs` - Add persistent metrics, analytics
- `src/config_file.rs` - Add MetricsConfig struct
- `src/lib.rs` - Export new types
- `src/main.rs` - Initialize metrics persistence

## Expected Metrics

- **New Code**: ~750 LOC
- **New Tests**: 50+ unit + 12 integration = 62 tests
- **Total Tests After Week 4**: 296+ (234 + 62)
- **Documentation**: ~500 lines

## Risks & Mitigation

| Risk | Mitigation |
| ------ | ----------- |
| File I/O performance | Async writes, batching |
| Config parsing complexity | Use serde, comprehensive validation |
| Prometheus format correctness | Reference official spec, validate output |
| Analytics calculation errors | Extensive unit tests, mathematical verification |
| Concurrent metric access | Use Arc<Mutex<>>, test under load |

## References

- Prometheus Exposition Format:
  <https://prometheus.io/docs/instrumenting/exposition_formats/>
- Systemd Service Documentation
- TOML Config Format
- JSON Serialization in Rust (serde)

## Next Steps (Phase 6 Week 5+)

1. Dashboard integration (Grafana templates)
2. Alerting system (threshold-based alerts)
3. Metrics query language (PromQL integration)
4. Time-series database integration (InfluxDB)
5. Historical trend analysis
6. Predictive recovery recommendations
