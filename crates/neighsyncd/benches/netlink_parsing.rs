//! Netlink message parsing benchmarks
//!
//! Measures the performance of parsing netlink neighbor messages.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

/// Simulate parsing a single netlink neighbor message
fn parse_single_neighbor() -> usize {
    // Simulate parsing overhead:
    // - Message header (16 bytes)
    // - Neighbor message (12 bytes)
    // - Attributes (variable, ~64-128 bytes typical, total ~192 bytes)

    // Simulate zero-copy parsing by reading message fields
    let mut parsed_bytes = 0;
    parsed_bytes += 16; // Header
    parsed_bytes += 12; // Neighbor message
    parsed_bytes += 64; // Attributes (IP, MAC, interface)

    parsed_bytes
}

/// Benchmark parsing a single neighbor message
fn bench_parse_single(c: &mut Criterion) {
    c.bench_function("parse_single_neighbor", |b| {
        b.iter(|| {
            let bytes = parse_single_neighbor();
            black_box(bytes);
        });
    });
}

/// Benchmark parsing batches of neighbor messages
fn bench_parse_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_neighbor_batch");

    for batch_size in [10, 50, 100, 500, 1000] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &size| {
                b.iter(|| {
                    let mut total_bytes = 0;
                    for _ in 0..size {
                        total_bytes += parse_single_neighbor();
                    }
                    black_box(total_bytes);
                });
            },
        );
    }

    group.finish();
}

/// Simulate interface index to name lookup (with caching)
fn lookup_interface_cached(ifindex: u32, cache_size: usize) -> String {
    // Simulate hash map lookup
    if ifindex < cache_size as u32 {
        format!("Ethernet{}", ifindex)
    } else {
        "unknown".to_string()
    }
}

/// Benchmark interface lookups with different cache sizes
fn bench_interface_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("interface_lookup");

    for cache_size in [16, 64, 256, 1024] {
        group.bench_with_input(
            BenchmarkId::from_parameter(cache_size),
            &cache_size,
            |b, &size| {
                b.iter(|| {
                    // Lookup 100 interfaces
                    for i in 0..100 {
                        let name = lookup_interface_cached(i % size as u32, size);
                        black_box(name);
                    }
                });
            },
        );
    }

    group.finish();
}

/// Simulate neighbor entry validation
fn validate_neighbor_entry(ip: &str, mac: &str, state: &str) -> bool {
    // Check if IP is valid format
    if !ip.contains(':') && !ip.contains('.') {
        return false;
    }

    // Check if MAC is valid format
    if mac.len() != 17 || mac.matches(':').count() != 5 {
        return false;
    }

    // Check state
    if !matches!(
        state,
        "Reachable" | "Stale" | "Delay" | "Probe" | "Permanent"
    ) {
        return false;
    }

    true
}

/// Benchmark neighbor entry validation
fn bench_validation(c: &mut Criterion) {
    c.bench_function("validate_neighbor_entry", |b| {
        b.iter(|| {
            let valid = validate_neighbor_entry(
                black_box("2001:db8::1"),
                black_box("00:11:22:33:44:55"),
                black_box("Reachable"),
            );
            black_box(valid);
        });
    });
}

/// Benchmark memory allocation patterns for event buffers
fn bench_buffer_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_allocation");

    // Pre-allocated buffer (reused)
    group.bench_function("preallocated", |b| {
        let mut buffer = Vec::with_capacity(128 * 192); // 128 messages
        b.iter(|| {
            buffer.clear();
            for i in 0..128 {
                buffer.push(i);
            }
            black_box(&buffer);
        });
    });

    // Dynamic allocation (per-call)
    group.bench_function("dynamic", |b| {
        b.iter(|| {
            let mut buffer = Vec::new();
            for i in 0..128 {
                buffer.push(i);
            }
            black_box(buffer);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_parse_single,
    bench_parse_batch,
    bench_interface_lookup,
    bench_validation,
    bench_buffer_allocation
);
criterion_main!(benches);
