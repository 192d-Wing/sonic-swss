# Telemetry Strategy Analysis: OpenTelemetry vs Prometheus-Direct

## Executive Summary

**Recommendation**: Use **Prometheus-Direct** approach for Phase 6 (now), with
OpenTelemetry readiness for Phase 7+

**Rationale**:

- Prometheus-direct: Simpler, faster, immediate operational value
- OpenTelemetry: Better for future complex observability, but adds unnecessary
  complexity now

---

## Option 1: Prometheus-Direct (Recommended for Phase 6)

### Architecture (Prometheus Pull)

```text
portsyncd
  ├─ Event Processing
  ├─ Metrics Collection (in-process)
  └─ /metrics HTTP Endpoint (Prometheus format)
       │
       └─ Prometheus Server scrapes /metrics
            │
            ├─ Grafana Dashboards
            ├─ Alert Rules
            └─ Time-Series Storage
```

### Implementation

```rust
// In Cargo.toml
[dependencies]
prometheus = "0.13"

// In src/metrics.rs
use prometheus::{Counter, Gauge, Histogram, Registry};

pub struct MetricsCollector {
    // Counters
    events_processed: Counter,
    events_failed: Counter,
    port_flaps: Counter,

    // Gauges
    queue_depth: Gauge,
    memory_bytes: Gauge,
    health_status: Gauge,

    // Histograms
    event_latency: Histogram,
    redis_latency: Histogram,

    registry: Registry,
}

impl MetricsCollector {
    pub fn new() -> Result<Self> {
        let registry = Registry::new();

        let events_processed = Counter::new("portsyncd_events_processed_total", "Total events processed")?;
        registry.register(Box::new(events_processed.clone()))?;

        let event_latency = Histogram::with_opts(
            HistogramOpts::new("portsyncd_event_latency_seconds", "Event processing latency"),
            &[0.001, 0.005, 0.01, 0.05, 0.1],
        )?;
        registry.register(Box::new(event_latency.clone()))?;

        Ok(Self {
            events_processed,
            event_latency,
            registry,
            // ...
        })
    }

    pub fn metrics_text(&self) -> String {
        let encoder = prometheus::TextEncoder::new();
        encoder.encode(&self.registry.gather(), &mut vec![]).unwrap()
    }
}

// In main.rs
async fn metrics_server(metrics: Arc<MetricsCollector>) {
    let app = axum::Router::new()
        .route("/metrics", axum::routing::get(|| async {
            metrics.metrics_text()
        }));

    axum::Server::bind(&"0.0.0.0:9090".parse()?)
        .serve(app.into_make_service())
        .await?;
}
```

### Pros (Prometheus Pull)

✅ **Simple & Fast**

- Single HTTP endpoint (99 lines of code)
- No external services required for metrics collection
- Negligible performance overhead

✅ **Standard Prometheus Format**

- Direct integration with Prometheus/Grafana ecosystem
- All existing alerting rules work
- Can reuse hundreds of community dashboards

✅ **Low Operational Complexity**

- No additional infrastructure (no Jaeger, no Collector)
- Prometheus scrapes on schedule (every 15-60s)
- Easy to operate and debug

✅ **Perfect for Daemon Monitoring**

- Designed for systems like portsyncd (long-running services)
- Metrics are aggregatable across fleet
- Handles metric cardinality well

✅ **Immediate Operational Value**

- Deploy and see results within hours
- Dashboards in Grafana the same day
- Alerting working immediately

### Cons (Prometheus Pull)

❌ **No Distributed Tracing**

- Can't trace request across multiple services
- (Not needed for portsyncd - single daemon)

❌ **Push Model Requires Workaround**

- If needed later, would require Prometheus Pushgateway
- (Unlikely for portsyncd daemon)

❌ **Limited Correlation**

- Can't correlate metrics with structured logs
- (Can be added later with sidecar logging)

---

## Option 2: OpenTelemetry + Prometheus Export (For Phase 7+)

### Architecture (OpenTelemetry)

```text
portsyncd
  ├─ Event Processing
  ├─ OpenTelemetry SDK (in-process)
  │  ├─ Metrics
  │  ├─ Traces
  │  └─ Logs
  └─ OTLP Exporter
       │
       ├─ Prometheus Exporter (for /metrics)
       ├─ Jaeger Exporter (for traces)
       └─ OpenTelemetry Collector
            │
            ├─ Prometheus
            ├─ Jaeger
            ├─ DataDog
            └─ Other backends
```

### Implementation Example

```rust
// In Cargo.toml
[dependencies]
opentelemetry = { version = "0.25", features = ["trace", "metrics"] }
opentelemetry_sdk = { version = "0.25", features = ["rt-tokio"] }
opentelemetry-prometheus = "0.15"
opentelemetry-jaeger = "0.16"
tracing-opentelemetry = "0.25"

// In src/telemetry.rs
use opentelemetry::global;
use opentelemetry_prometheus::PrometheusBuilder;
use opentelemetry_sdk::metrics::MeterProvider;
use opentelemetry_jaeger::new_pipeline;

pub fn init_telemetry() -> Result<MeterProvider> {
    // Prometheus metrics exporter
    let prometheus_exporter = PrometheusBuilder::new()
        .with_registry(prometheus::Registry::new())
        .build()?;

    let meter_provider = MeterProvider::builder()
        .with_reader(prometheus_exporter)
        .build();

    global::set_meter_provider(meter_provider.clone());

    // Jaeger trace exporter (optional)
    let tracer = new_pipeline()
        .install_simple()?;

    global::set_tracer_provider(tracer);

    Ok(meter_provider)
}

// Usage
let meter = global::meter("portsyncd");
let event_counter = meter.u64_counter("events_processed").init();
event_counter.add(1, &[]);
```

### Pros (OpenTelemetry)

✅ **Future-Proof Architecture**

- Single telemetry SDK handles metrics, traces, logs
- Can add distributed tracing later
- Supports multiple export backends
- Industry standard (Cloud Native Computing Foundation)

✅ **Distributed Tracing Capable**

- Can trace events across multiple services
- Correlate logs with metrics with traces
- RCA (Root Cause Analysis) becomes easier

✅ **Multi-Backend Support**

- Export to Prometheus, Jaeger, DataDog, Honeycomb, etc.
- Switch backends without code changes
- Vendor-agnostic

✅ **Structured Logging Integration**

- Same instrumentation API for metrics, logs, traces
- Unified observability

### Cons (OpenTelemetry)

❌ **Higher Complexity**

- 200+ lines of code for full setup
- More dependencies to maintain
- Harder to debug if something goes wrong

❌ **Unnecessary for Current Needs**

- portsyncd is single daemon (no distributed context)
- Tracing overhead without clear benefit now
- Adds operational complexity (Collector, Jaeger)

❌ **Dependency Bloat**

- ~10 additional crates for full OpenTelemetry setup
- Increases binary size
- More potential security issues to track

❌ **Learning Curve**

- OpenTelemetry API is more complex
- Team needs training
- Debugging is harder

❌ **Performance Overhead**

- Additional CPU and memory for SDK
- Serialization to OTLP format
- Network calls to Collector

❌ **Operational Burden**

- Must run OpenTelemetry Collector
- Must run Jaeger backend (if using traces)
- More services to monitor and maintain

---

## Option 3: Hybrid Approach (Best of Both Worlds)

### Phase 6: Prometheus-Direct

- Simple, fast, immediate value
- ~100 lines of code
- Deploy and get results in days

### Phase 7: Add OpenTelemetry Readiness

- Keep existing Prometheus metrics
- Add OpenTelemetry SDK alongside
- Enable tracing without breaking metrics
- No code changes to existing metrics

### Architecture (Hybrid)

```text
portsyncd
  ├─ Prometheus Metrics (Phase 6)
  │  └─ /metrics endpoint
  │
  ├─ OpenTelemetry Tracing (Phase 7, optional)
  │  └─ OTLP Exporter to Collector
  │
  └─ Structured Logging (Phase 7+)
     └─ JSONLines to stdout
```

### Code Structure

```rust
// src/metrics.rs - Keep simple
pub struct PrometheusMetrics { ... }

// src/telemetry.rs - New module
pub struct OpenTelemetryTracer {
    tracer: Tracer,
}

// In main.rs
let metrics = PrometheusMetrics::new()?;
let tracer = if config.enable_tracing {
    Some(OpenTelemetryTracer::new()?)
} else {
    None
};
```

---

## Comparison Matrix

| Aspect | Prometheus-Direct | OpenTelemetry | Hybrid |
| -------- | ------------------- | --------------- | -------- |
| **Implementation Time** | 4 hours | 20+ hours | 4 hrs now, 20 hrs later |
| **Operational Complexity** | Low | High | Low → Medium |
| **Dependencies** | 1 crate | 10+ crates | 1 → 11 crates |
| **Phase 6 Value** | 100% | 70% | 100% |
| **Future Extensibility** | 60% | 100% | 100% |
| **Immediate Results** | ✅ Yes | ✅ Yes | ✅ Yes |
| **Distributed Tracing** | ❌ No | ✅ Yes | Phase 7+ |
| **Multi-Backend Export** | ❌ No | ✅ Yes | Phase 7+ |
| **Performance Overhead** | Minimal | 5-10% | Minimal now |

---

## Recommendation by Deployment Scenario

### Small Deployment (< 50 switches)

**Choose**: Prometheus-Direct

- Simplicity is paramount
- Distributed tracing not needed
- Quick time-to-value

### Medium Deployment (50-200 switches)

**Choose**: Prometheus-Direct now, plan Hybrid for Phase 7

- Need fast deployment
- May want tracing for complex issues later
- Leave path open for evolution

### Large Deployment (200+ switches)

**Choose**: Hybrid Approach

- Invest in Prometheus-Direct now
- Plan OpenTelemetry for Phase 7
- Prepare team for distributed tracing later

### Carrier-Grade (SLA-critical, multiple services)

**Choose**: OpenTelemetry + Prometheus (Jump to Phase 7 level)

- Full observability from day one
- Already have distributed services
- Justify operational overhead

---

## Implementation Decision Tree

```text
Does portsyncd need to correlate metrics with
traces from other services?
  ├─ NO (single daemon, no external dependencies)
  │   └─ → Use Prometheus-Direct (Phase 6)
  │
  └─ YES (part of larger system with orchagent, etc.)
      ├─ Do you have operational budget for Collector/Jaeger?
      │   ├─ NO → Use Prometheus-Direct now, add OpenTelemetry Phase 7
      │   │       └─ → Hybrid Approach
      │   │
      │   └─ YES → Use OpenTelemetry + Prometheus
      │            └─ → Full OpenTelemetry (Phase 7+)
```

---

## Recommended Path Forward

### Phase 6 Week 1: Prometheus-Direct Implementation

```text
Monday-Wednesday:
  ├─ Create metrics.rs with PrometheusMetrics struct
  ├─ Add prometheus crate to Cargo.toml
  ├─ Implement metrics collection in event processing
  └─ Add HTTP server for /metrics endpoint

Thursday:
  ├─ Create Grafana dashboard
  └─ Test with Prometheus scraping

Friday:
  ├─ Documentation
  └─ Deploy and verify
```

**Code Estimate**: ~100-150 lines
**Testing Time**: ~4-6 hours total

### Phase 7: Add OpenTelemetry (If Needed)

When requirements emerge for:

- Distributed tracing across SONiC services
- Correlation with orchagent, swsscommon, etc.
- Advanced debugging across daemon boundaries

Then:

1. Keep existing Prometheus metrics (no changes)
2. Add OpenTelemetry SDK alongside
3. Implement tracing for request paths
4. Deploy Collector and Jaeger
5. Gradually adopt structured logging

---

## Code Skeleton for Phase 6 (Prometheus-Direct)

### src/metrics.rs

```rust
use prometheus::{Counter, Gauge, Histogram, HistogramOpts, Registry};
use std::sync::Arc;

#[derive(Clone)]
pub struct MetricsCollector {
    // Counters
    pub events_processed: Counter,
    pub events_failed: Counter,
    pub port_flaps: Counter,

    // Gauges
    pub queue_depth: Gauge,
    pub memory_bytes: Gauge,
    pub health_status: Gauge,

    // Histograms
    pub event_latency_secs: Histogram,
    pub redis_latency_secs: Histogram,

    registry: Arc<Registry>,
}

impl MetricsCollector {
    pub fn new() -> Result<Self> {
        let registry = Registry::new();

        // Create and register metrics
        let events_processed = Counter::new(
            "portsyncd_events_processed_total",
            "Total events processed successfully"
        )?;
        registry.register(Box::new(events_processed.clone()))?;

        let event_latency_secs = Histogram::with_opts(
            HistogramOpts::new("portsyncd_event_latency_seconds", "Event processing latency"),
            &[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0],
        )?;
        registry.register(Box::new(event_latency_secs.clone()))?;

        // ... more metrics

        Ok(Self {
            events_processed,
            event_latency_secs,
            registry: Arc::new(registry),
        })
    }

    pub fn gather_metrics(&self) -> String {
        let encoder = prometheus::TextEncoder::new();
        encoder.encode(&self.registry.gather(), &mut vec![])
            .unwrap_or_else(|_| String::from("# Error encoding metrics\n"))
    }
}
```

### In main.rs

```rust
// Initialize metrics
let metrics = Arc::new(MetricsCollector::new()?);

// Start metrics HTTP server
let metrics_clone = metrics.clone();
tokio::spawn(async move {
    start_metrics_server(metrics_clone).await
});

// Use metrics in event processing
metrics.events_processed.inc();
let timer = metrics.event_latency_secs.start_timer();
// ... process event
timer.observe_duration();
```

---

## Final Recommendation

**For Phase 6**: Use **Prometheus-Direct**

- ✅ 4-6 hour implementation
- ✅ Immediate operational value
- ✅ Standard Prometheus ecosystem
- ✅ No operational overhead
- ✅ Proven approach for daemon monitoring

**Reserve OpenTelemetry for Phase 7+ when you need:**

- Distributed tracing across multiple services
- Correlation with other SONiC daemons
- Multi-backend telemetry export
- Advanced observability features

This approach:

1. Gets you operational dashboards and alerting **this week**
2. Keeps complexity **manageable**
3. Leaves door **open for OpenTelemetry when truly needed**
4. Maximizes **time-to-value**
5. Minimizes **operational burden**

---

## References

- Prometheus Best Practices: <https://prometheus.io/docs/practices/>
- Prometheus Rust Client: <https://docs.rs/prometheus/>
- OpenTelemetry Documentation: <https://opentelemetry.io/docs/>
- CNCF Observability Guide:
  <https://www.cncf.io/blog/2022/04/12/what-does-observability-mean/>
- SONiC Monitoring: <https://github.com/sonic-net/SONiC/wiki/Monitoring>
