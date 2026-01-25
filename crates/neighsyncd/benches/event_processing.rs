//! Full event processing pipeline benchmarks
//!
//! Measures end-to-end performance of the neighbor event processing pipeline.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;

/// Simulate a complete neighbor event from netlink to Redis
#[derive(Clone)]
struct NeighborEvent {
    #[allow(dead_code)]
    ifindex: u32,
    #[allow(dead_code)]
    interface: String,
    ip: String,
    mac: String,
    #[allow(dead_code)]
    state: String,
}

impl NeighborEvent {
    fn new(index: usize) -> Self {
        Self {
            ifindex: (index % 256) as u32,
            interface: format!("Ethernet{}", index % 64),
            ip: format!("2001:db8::{:x}", index),
            mac: format!(
                "00:11:22:33:{:02x}:{:02x}",
                (index >> 8) & 0xff,
                index & 0xff
            ),
            state: "Reachable".to_string(),
        }
    }
}

/// Simulate parsing a netlink message
fn parse_netlink_message(raw_data: &[u8]) -> Option<NeighborEvent> {
    if raw_data.len() < 64 {
        return None;
    }

    // Simulate parsing overhead
    let index = raw_data[0] as usize;
    Some(NeighborEvent::new(index))
}

/// Simulate validation and filtering
fn validate_and_filter(event: &NeighborEvent) -> bool {
    // Check for broadcast MAC
    if event.mac == "ff:ff:ff:ff:ff:ff" {
        return false;
    }

    // Check for zero MAC (non-dual-tor)
    if event.mac == "00:00:00:00:00:00" {
        return false;
    }

    // Check for multicast link-local
    if event.ip.starts_with("ff02::") {
        return false;
    }

    true
}

/// Simulate Redis operation
fn write_to_redis(event: &NeighborEvent) -> Duration {
    black_box(event);
    // Simulate 500 microsecond latency
    Duration::from_micros(500)
}

/// Full pipeline: parse -> validate -> write
fn process_single_event(raw_data: &[u8]) -> Option<Duration> {
    let event = parse_netlink_message(raw_data)?;

    if !validate_and_filter(&event) {
        return None;
    }

    Some(write_to_redis(&event))
}

/// Benchmark single event processing
fn bench_single_event(c: &mut Criterion) {
    let raw_data = vec![42u8; 192]; // 192 byte netlink message

    c.bench_function("process_single_event", |b| {
        b.iter(|| {
            let latency = process_single_event(black_box(&raw_data));
            black_box(latency);
        });
    });
}

/// Benchmark batch processing pipeline
fn bench_batch_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_processing");

    for batch_size in [10, 50, 100, 500] {
        group.throughput(Throughput::Elements(batch_size as u64));

        // Generate batch of events
        let events: Vec<Vec<u8>> = (0..batch_size)
            .map(|i| {
                let mut data = vec![0u8; 192];
                data[0] = (i % 256) as u8;
                data
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &events,
            |b, events| {
                b.iter(|| {
                    let mut total_latency = Duration::ZERO;
                    for event_data in events {
                        if let Some(latency) = process_single_event(event_data) {
                            total_latency += latency;
                        }
                    }
                    black_box(total_latency);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark filtering overhead
fn bench_filtering(c: &mut Criterion) {
    let mut group = c.benchmark_group("filtering");

    // Valid event
    let valid_event = NeighborEvent::new(1);
    group.bench_function("valid_event", |b| {
        b.iter(|| {
            let result = validate_and_filter(black_box(&valid_event));
            black_box(result);
        });
    });

    // Broadcast MAC (filtered)
    let mut broadcast_event = NeighborEvent::new(2);
    broadcast_event.mac = "ff:ff:ff:ff:ff:ff".to_string();
    group.bench_function("broadcast_mac", |b| {
        b.iter(|| {
            let result = validate_and_filter(black_box(&broadcast_event));
            black_box(result);
        });
    });

    // Zero MAC (filtered)
    let mut zero_mac_event = NeighborEvent::new(3);
    zero_mac_event.mac = "00:00:00:00:00:00".to_string();
    group.bench_function("zero_mac", |b| {
        b.iter(|| {
            let result = validate_and_filter(black_box(&zero_mac_event));
            black_box(result);
        });
    });

    // Multicast link-local (filtered)
    let mut multicast_event = NeighborEvent::new(4);
    multicast_event.ip = "ff02::1".to_string();
    group.bench_function("multicast_link_local", |b| {
        b.iter(|| {
            let result = validate_and_filter(black_box(&multicast_event));
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark error handling paths
fn bench_error_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_handling");

    // Valid message
    let valid_data = vec![42u8; 192];
    group.bench_function("valid_message", |b| {
        b.iter(|| {
            let result = parse_netlink_message(black_box(&valid_data));
            black_box(result);
        });
    });

    // Truncated message (error case)
    let truncated_data = vec![42u8; 32];
    group.bench_function("truncated_message", |b| {
        b.iter(|| {
            let result = parse_netlink_message(black_box(&truncated_data));
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark throughput at different event rates
fn bench_throughput_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_simulation");

    for events_per_sec in [100, 500, 1000, 5000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(events_per_sec),
            &events_per_sec,
            |b, &rate| {
                // Simulate 1 second of events
                let events: Vec<Vec<u8>> = (0..rate)
                    .map(|i| {
                        let mut data = vec![0u8; 192];
                        data[0] = (i % 256) as u8;
                        data
                    })
                    .collect();

                b.iter(|| {
                    let mut processed = 0;
                    for event_data in &events {
                        if process_single_event(event_data).is_some() {
                            processed += 1;
                        }
                    }
                    black_box(processed);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark with mixed event types
fn bench_mixed_workload(c: &mut Criterion) {
    c.bench_function("mixed_workload", |b| {
        // 60% valid, 20% broadcast, 10% zero MAC, 10% multicast
        let events: Vec<NeighborEvent> = (0..100)
            .map(|i| {
                let mut event = NeighborEvent::new(i);
                match i % 10 {
                    0..=5 => {
                        // Valid (60%)
                    }
                    6..=7 => {
                        // Broadcast (20%)
                        event.mac = "ff:ff:ff:ff:ff:ff".to_string();
                    }
                    8 => {
                        // Zero MAC (10%)
                        event.mac = "00:00:00:00:00:00".to_string();
                    }
                    9 => {
                        // Multicast (10%)
                        event.ip = "ff02::1".to_string();
                    }
                    _ => unreachable!(),
                }
                event
            })
            .collect();

        b.iter(|| {
            let mut processed = 0;
            for event in &events {
                if validate_and_filter(event) {
                    let _ = write_to_redis(event);
                    processed += 1;
                }
            }
            black_box(processed);
        });
    });
}

/// Benchmark latency percentiles
fn bench_latency_distribution(c: &mut Criterion) {
    c.bench_function("latency_distribution", |b| {
        let events: Vec<Vec<u8>> = (0..1000)
            .map(|i| {
                let mut data = vec![0u8; 192];
                data[0] = (i % 256) as u8;
                data
            })
            .collect();

        b.iter(|| {
            let mut latencies = Vec::new();
            for event_data in &events {
                if let Some(latency) = process_single_event(event_data) {
                    latencies.push(latency);
                }
            }

            // Calculate p50, p95, p99
            latencies.sort();
            let p50 = latencies[latencies.len() / 2];
            let p95 = latencies[latencies.len() * 95 / 100];
            let p99 = latencies[latencies.len() * 99 / 100];

            black_box((p50, p95, p99));
        });
    });
}

criterion_group!(
    benches,
    bench_single_event,
    bench_batch_processing,
    bench_filtering,
    bench_error_handling,
    bench_throughput_simulation,
    bench_mixed_workload,
    bench_latency_distribution
);
criterion_main!(benches);
