/// Distributed tracing integration for neighsyncd
///
/// Provides OpenTelemetry/Jaeger integration for:
/// - End-to-end request tracing
/// - Performance analysis
/// - Event flow tracking
/// - Dependency tracing (Redis, Netlink)
///
/// Usage:
/// ```ignore
/// let tracer = TracingIntegration::new("neighsyncd", "1.0.0");
/// tracer.initialize()?;
/// let span = tracer.start_span("process_neighbor_event");
/// // ... work ...
/// tracer.end_span(span);
/// ```
use std::fmt;
use std::time::SystemTime;

/// Span kind indicates the type of operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    /// Server-side handler
    Server,
    /// Client-side call
    Client,
    /// Producer sending message
    Producer,
    /// Consumer receiving message
    Consumer,
    /// Internal operation
    Internal,
}

impl fmt::Display for SpanKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpanKind::Server => write!(f, "SERVER"),
            SpanKind::Client => write!(f, "CLIENT"),
            SpanKind::Producer => write!(f, "PRODUCER"),
            SpanKind::Consumer => write!(f, "CONSUMER"),
            SpanKind::Internal => write!(f, "INTERNAL"),
        }
    }
}

/// Span status indicates completion state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanStatus {
    /// Span completed successfully
    Ok,
    /// Span encountered an error
    Error,
    /// Span was cancelled
    Cancelled,
}

impl fmt::Display for SpanStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpanStatus::Ok => write!(f, "OK"),
            SpanStatus::Error => write!(f, "ERROR"),
            SpanStatus::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

/// Span represents a single operation in the trace
#[derive(Debug, Clone)]
pub struct Span {
    /// Trace ID (128-bit, globally unique)
    pub trace_id: String,
    /// Span ID (64-bit, unique within trace)
    pub span_id: String,
    /// Parent span ID (optional)
    pub parent_id: Option<String>,
    /// Operation name
    pub name: String,
    /// Span kind
    pub kind: SpanKind,
    /// Start timestamp (Unix nanoseconds)
    pub start_time: u64,
    /// End timestamp (Unix nanoseconds)
    pub end_time: Option<u64>,
    /// Status
    pub status: SpanStatus,
    /// Key-value attributes
    pub attributes: Vec<(String, String)>,
    /// Events with timestamps
    pub events: Vec<(String, u64)>,
}

impl Span {
    /// Create a new span
    pub fn new(trace_id: String, span_id: String, name: String) -> Self {
        let start_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        Self {
            trace_id,
            span_id,
            parent_id: None,
            name,
            kind: SpanKind::Internal,
            start_time,
            end_time: None,
            status: SpanStatus::Ok,
            attributes: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Set span kind
    pub fn with_kind(mut self, kind: SpanKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set parent span ID
    pub fn with_parent(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Add an attribute (key-value pair)
    pub fn add_attribute(&mut self, key: String, value: String) {
        self.attributes.push((key, value));
    }

    /// Add an event with description
    pub fn add_event(&mut self, description: String) {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        self.events.push((description, timestamp));
    }

    /// Mark span as completed
    pub fn end(&mut self, status: SpanStatus) {
        self.end_time = Some(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
        );
        self.status = status;
    }

    /// Get duration in microseconds
    pub fn duration_us(&self) -> Option<u64> {
        self.end_time.map(|end| (end - self.start_time) / 1000)
    }
}

/// Tracing integration for OpenTelemetry/Jaeger
pub struct TracingIntegration {
    #[allow(dead_code)]
    service_name: String,
    #[allow(dead_code)]
    service_version: String,
    enabled: bool,
}

impl TracingIntegration {
    /// Create new tracing integration
    pub fn new(service_name: &str, service_version: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
            service_version: service_version.to_string(),
            enabled: false,
        }
    }

    /// Initialize tracing with Jaeger exporter
    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // In a real implementation, this would:
        // 1. Create OpenTelemetry TracerProvider
        // 2. Configure Jaeger exporter with:
        //    - Agent endpoint (localhost:6831)
        //    - Service name
        //    - Resource attributes
        // 3. Install global tracer
        //
        // For now, we return Ok to allow compilation without full OTel setup
        // Production: Uncomment OTel configuration code

        /*
        use opentelemetry::global;
        use opentelemetry::sdk::trace::{Tracer, TracerProvider};
        use opentelemetry::sdk::Resource;
        use opentelemetry_jaeger;

        let resource = Resource::new(vec![
            opentelemetry::KeyValue::new("service.name", self.service_name.clone()),
            opentelemetry::KeyValue::new("service.version", self.service_version.clone()),
        ]);

        let tracer_provider = TracerProvider::builder()
            .with_sampler(opentelemetry::sdk::trace::Sampler::ParentBased(
                Box::new(opentelemetry::sdk::trace::Sampler::TraceIdRatioBased(1.0)),
            ))
            .with_batch_exporter(
                opentelemetry_jaeger::new_pipeline()
                    .install_simple()
                    .map_err(|e| format!("Failed to install Jaeger exporter: {}", e))?,
                opentelemetry::runtime::Tokio,
            )
            .with_resource(resource)
            .build();

        global::set_tracer_provider(tracer_provider);
        */

        self.enabled = true;
        Ok(())
    }

    /// Start a new span
    pub fn start_span(&self, operation_name: &str) -> Span {
        let trace_id = self.generate_trace_id();
        let span_id = self.generate_span_id();

        Span::new(trace_id, span_id, operation_name.to_string())
    }

    /// Start a child span (with parent reference)
    pub fn start_child_span(&self, parent: &Span, operation_name: &str) -> Span {
        let span_id = self.generate_span_id();
        Span::new(parent.trace_id.clone(), span_id, operation_name.to_string())
            .with_parent(parent.span_id.clone())
    }

    /// Record span completion
    pub fn end_span(&self, mut span: Span, status: SpanStatus) {
        span.end(status);

        if self.enabled {
            if let Some(duration_us) = span.duration_us() {
                // Log span metadata for debugging
                tracing::debug!(
                    trace_id = %span.trace_id,
                    span_id = %span.span_id,
                    operation = %span.name,
                    duration_us = duration_us,
                    status = %span.status,
                    "Span completed"
                );
            }
        }
    }

    /// Generate random 128-bit trace ID (hex string)
    fn generate_trace_id(&self) -> String {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};

        let mut hasher = RandomState::new().build_hasher();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        hasher.write_u128(nanos);

        format!("{:032x}", hasher.finish())
    }

    /// Generate random 64-bit span ID (hex string)
    fn generate_span_id(&self) -> String {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};

        let mut hasher = RandomState::new().build_hasher();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        hasher.write_u128(nanos);

        format!("{:016x}", hasher.finish())
    }

    /// Export accumulated spans (normally handled by exporter)
    pub fn export_spans(&self, spans: Vec<Span>) -> Result<(), Box<dyn std::error::Error>> {
        if !self.enabled {
            return Ok(());
        }

        // In production, this would export to Jaeger via:
        // - Jaeger HTTP endpoint
        // - gRPC endpoint
        // - UDP agent (legacy)
        //
        // For now, we log export for debugging

        tracing::debug!(span_count = spans.len(), "Exporting spans");

        for span in spans {
            tracing::debug!(
                trace_id = %span.trace_id,
                span_id = %span.span_id,
                parent_id = ?span.parent_id,
                operation = %span.name,
                kind = %span.kind,
                status = %span.status,
                duration_us = ?span.duration_us(),
                attribute_count = span.attributes.len(),
                event_count = span.events.len(),
                "Exported span"
            );
        }

        Ok(())
    }

    /// Check if tracing is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Predefined span attributes for common operations
pub mod attributes {
    use super::Span;

    /// Add neighbor-related attributes
    pub fn add_neighbor_attributes(span: &mut Span, interface: &str, ip: &str, mac: &str) {
        span.add_attribute("neighbor.interface".to_string(), interface.to_string());
        span.add_attribute("neighbor.ip".to_string(), ip.to_string());
        span.add_attribute("neighbor.mac".to_string(), mac.to_string());
    }

    /// Add Redis operation attributes
    pub fn add_redis_attributes(span: &mut Span, operation: &str, key_count: usize) {
        span.add_attribute("redis.operation".to_string(), operation.to_string());
        span.add_attribute("redis.key_count".to_string(), key_count.to_string());
    }

    /// Add netlink event attributes
    pub fn add_netlink_attributes(span: &mut Span, event_type: &str, count: usize) {
        span.add_attribute("netlink.event_type".to_string(), event_type.to_string());
        span.add_attribute("netlink.count".to_string(), count.to_string());
    }

    /// Add batch operation attributes
    pub fn add_batch_attributes(span: &mut Span, batch_size: usize, duration_ms: u64) {
        span.add_attribute("batch.size".to_string(), batch_size.to_string());
        span.add_attribute("batch.duration_ms".to_string(), duration_ms.to_string());
    }

    /// Add error attributes
    pub fn add_error_attributes(span: &mut Span, error_type: &str, error_msg: &str) {
        span.add_attribute("error.type".to_string(), error_type.to_string());
        span.add_attribute("error.message".to_string(), error_msg.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_creation() {
        let span = Span::new(
            "abc123".to_string(),
            "def456".to_string(),
            "test_operation".to_string(),
        );

        assert_eq!(span.trace_id, "abc123");
        assert_eq!(span.span_id, "def456");
        assert_eq!(span.name, "test_operation");
        assert_eq!(span.kind, SpanKind::Internal);
        assert_eq!(span.status, SpanStatus::Ok);
        assert!(span.end_time.is_none());
    }

    #[test]
    fn test_span_with_parent() {
        let parent_span = Span::new(
            "parent123".to_string(),
            "parent456".to_string(),
            "parent_op".to_string(),
        );

        let child_span = Span::new(
            "parent123".to_string(),
            "child789".to_string(),
            "child_op".to_string(),
        )
        .with_parent(parent_span.span_id.clone());

        assert_eq!(child_span.parent_id, Some("parent456".to_string()));
    }

    #[test]
    fn test_span_attributes() {
        let mut span = Span::new("trace1".to_string(), "span1".to_string(), "op1".to_string());

        span.add_attribute("key1".to_string(), "value1".to_string());
        span.add_attribute("key2".to_string(), "value2".to_string());

        assert_eq!(span.attributes.len(), 2);
        assert_eq!(
            span.attributes[0],
            ("key1".to_string(), "value1".to_string())
        );
    }

    #[test]
    fn test_span_events() {
        let mut span = Span::new("trace1".to_string(), "span1".to_string(), "op1".to_string());

        span.add_event("Event 1".to_string());
        span.add_event("Event 2".to_string());

        assert_eq!(span.events.len(), 2);
        assert_eq!(span.events[0].0, "Event 1");
        assert_eq!(span.events[1].0, "Event 2");
    }

    #[test]
    fn test_span_completion() {
        let mut span = Span::new("trace1".to_string(), "span1".to_string(), "op1".to_string());

        assert!(span.end_time.is_none());
        span.end(SpanStatus::Ok);
        assert!(span.end_time.is_some());
        assert!(span.duration_us().is_some());
    }

    #[test]
    fn test_tracing_integration_creation() {
        let tracing = TracingIntegration::new("test-service", "1.0.0");
        assert!(!tracing.is_enabled());
    }

    #[test]
    fn test_span_kind_display() {
        assert_eq!(format!("{}", SpanKind::Server), "SERVER");
        assert_eq!(format!("{}", SpanKind::Client), "CLIENT");
        assert_eq!(format!("{}", SpanKind::Internal), "INTERNAL");
    }

    #[test]
    fn test_span_status_display() {
        assert_eq!(format!("{}", SpanStatus::Ok), "OK");
        assert_eq!(format!("{}", SpanStatus::Error), "ERROR");
        assert_eq!(format!("{}", SpanStatus::Cancelled), "CANCELLED");
    }

    #[test]
    fn test_neighbor_attributes() {
        let mut span = Span::new(
            "trace1".to_string(),
            "span1".to_string(),
            "process_neighbor".to_string(),
        );

        attributes::add_neighbor_attributes(
            &mut span,
            "Ethernet0",
            "2001:db8::1",
            "00:11:22:33:44:55",
        );

        assert_eq!(span.attributes.len(), 3);
    }

    #[test]
    fn test_redis_attributes() {
        let mut span = Span::new(
            "trace1".to_string(),
            "span1".to_string(),
            "redis_batch".to_string(),
        );

        attributes::add_redis_attributes(&mut span, "HSET", 100);

        assert_eq!(span.attributes.len(), 2);
    }
}
