//! Redis operations benchmarks
//!
//! Measures the performance of Redis batch operations and pipelining.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

/// Simulate Redis round-trip latency
const REDIS_LATENCY_MS: f64 = 0.5; // 500 microseconds

/// Simulate single Redis SET operation
fn redis_set_single(key: &str, value: &str) -> Duration {
    // Simulate command serialization + network + processing
    black_box(key);
    black_box(value);
    Duration::from_secs_f64(REDIS_LATENCY_MS / 1000.0)
}

/// Simulate batched Redis operations with pipelining
fn redis_set_batch_pipelined(entries: &[(String, String)]) -> Duration {
    // Pipelining: single round-trip for entire batch
    black_box(entries);
    Duration::from_secs_f64(REDIS_LATENCY_MS / 1000.0)
}

/// Benchmark single vs batched Redis operations
fn bench_redis_single_vs_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("redis_operations");

    for count in [10, 50, 100, 500] {
        group.throughput(Throughput::Elements(count as u64));

        // Single operations
        group.bench_with_input(
            BenchmarkId::new("single", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let mut total_latency = Duration::ZERO;
                    for i in 0..count {
                        let key = format!("NEIGH_TABLE:eth0:2001:db8::{}", i);
                        let value = format!("00:11:22:33:44:{:02x}", i % 256);
                        total_latency += redis_set_single(&key, &value);
                    }
                    black_box(total_latency);
                });
            },
        );

        // Batched operations with pipelining
        group.bench_with_input(
            BenchmarkId::new("batched", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let entries: Vec<(String, String)> = (0..count)
                        .map(|i| {
                            (
                                format!("NEIGH_TABLE:eth0:2001:db8::{}", i),
                                format!("00:11:22:33:44:{:02x}", i % 256),
                            )
                        })
                        .collect();
                    let latency = redis_set_batch_pipelined(&entries);
                    black_box(latency);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark batch size efficiency
fn bench_batch_size_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_size");
    let total_operations = 1000;

    for batch_size in [1, 10, 50, 100, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &size| {
                b.iter(|| {
                    let batch_count = (total_operations + size - 1) / size;
                    let mut total_latency = Duration::ZERO;

                    for batch_idx in 0..batch_count {
                        let batch_start = batch_idx * size;
                        let batch_end = std::cmp::min(batch_start + size, total_operations);
                        let batch_items = batch_end - batch_start;

                        let entries: Vec<(String, String)> = (0..batch_items)
                            .map(|i| {
                                (
                                    format!("key_{}", batch_start + i),
                                    format!("value_{}", batch_start + i),
                                )
                            })
                            .collect();

                        total_latency += redis_set_batch_pipelined(&entries);
                    }

                    black_box(total_latency);
                });
            },
        );
    }

    group.finish();
}

/// Simulate Redis hash operations (HSET)
fn redis_hset(key: &str, fields: &[(String, String)]) -> Duration {
    black_box(key);
    black_box(fields);
    Duration::from_secs_f64(REDIS_LATENCY_MS / 1000.0)
}

/// Benchmark hash operations vs string operations
fn bench_hash_vs_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("redis_data_structures");

    // String operations (separate keys)
    group.bench_function("strings", |b| {
        b.iter(|| {
            let neighbor_key = "NEIGH_TABLE:eth0:2001:db8::1";
            redis_set_single(&format!("{}:neigh", neighbor_key), "00:11:22:33:44:55");
            redis_set_single(&format!("{}:family", neighbor_key), "IPv6");
            redis_set_single(&format!("{}:state", neighbor_key), "Reachable");
        });
    });

    // Hash operations (single key with fields)
    group.bench_function("hashes", |b| {
        b.iter(|| {
            let neighbor_key = "NEIGH_TABLE:eth0:2001:db8::1";
            let fields = vec![
                ("neigh".to_string(), "00:11:22:33:44:55".to_string()),
                ("family".to_string(), "IPv6".to_string()),
                ("state".to_string(), "Reachable".to_string()),
            ];
            redis_hset(neighbor_key, &fields);
        });
    });

    group.finish();
}

/// Benchmark Redis key format overhead
fn bench_key_format(c: &mut Criterion) {
    let mut group = c.benchmark_group("key_format");

    group.bench_function("simple", |b| {
        b.iter(|| {
            for i in 0..100 {
                let key = format!("key_{}", i);
                black_box(key);
            }
        });
    });

    group.bench_function("neighbor_format", |b| {
        b.iter(|| {
            for i in 0..100 {
                let key = format!("NEIGH_TABLE:eth0:2001:db8::{:x}", i);
                black_box(key);
            }
        });
    });

    group.finish();
}

/// Benchmark connection pool vs single connection
fn bench_connection_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("connection_patterns");

    // Single connection (sequential)
    group.bench_function("single_connection", |b| {
        b.iter(|| {
            let mut total_latency = Duration::ZERO;
            for i in 0..10 {
                let key = format!("key_{}", i);
                total_latency += redis_set_single(&key, "value");
            }
            black_box(total_latency);
        });
    });

    // Pipelined on single connection
    group.bench_function("pipelined", |b| {
        b.iter(|| {
            let entries: Vec<(String, String)> = (0..10)
                .map(|i| (format!("key_{}", i), "value".to_string()))
                .collect();
            let latency = redis_set_batch_pipelined(&entries);
            black_box(latency);
        });
    });

    group.finish();
}

/// Benchmark memory allocation for Redis commands
fn bench_command_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("command_allocation");

    // Pre-allocated command buffer
    group.bench_function("preallocated", |b| {
        let mut buffer = String::with_capacity(1024);
        b.iter(|| {
            buffer.clear();
            for i in 0..10 {
                buffer.push_str("SET key_");
                buffer.push_str(&i.to_string());
                buffer.push_str(" value\n");
            }
            black_box(&buffer);
        });
    });

    // Dynamic allocation
    group.bench_function("dynamic", |b| {
        b.iter(|| {
            let mut commands = Vec::new();
            for i in 0..10 {
                let cmd = format!("SET key_{} value\n", i);
                commands.push(cmd);
            }
            black_box(commands);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_redis_single_vs_batch,
    bench_batch_size_efficiency,
    bench_hash_vs_string,
    bench_key_format,
    bench_connection_patterns,
    bench_command_allocation
);
criterion_main!(benches);
