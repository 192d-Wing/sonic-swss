//! Warm restart reconciliation benchmarks
//!
//! Measures the performance of warm restart state caching and reconciliation.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::time::Duration;

/// Neighbor entry for caching
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct NeighborKey {
    interface: String,
    ip: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NeighborValue {
    mac: String,
    state: String,
}

/// Simulate loading cached neighbors from Redis
fn load_cached_neighbors(count: usize) -> HashMap<NeighborKey, NeighborValue> {
    (0..count)
        .map(|i| {
            let key = NeighborKey {
                interface: format!("Ethernet{}", i % 64),
                ip: format!("2001:db8::{:x}", i),
            };
            let value = NeighborValue {
                mac: format!("00:11:22:33:{:02x}:{:02x}", (i >> 8) & 0xff, i & 0xff),
                state: "Reachable".to_string(),
            };
            (key, value)
        })
        .collect()
}

/// Simulate loading kernel neighbors from netlink
fn load_kernel_neighbors(count: usize) -> HashMap<NeighborKey, NeighborValue> {
    // 80% overlap with cached, 20% new
    (0..count)
        .map(|i| {
            // Offset by 20% to create partial overlap
            let offset = if i < count * 80 / 100 { i } else { i + 1000 };

            let key = NeighborKey {
                interface: format!("Ethernet{}", offset % 64),
                ip: format!("2001:db8::{:x}", offset),
            };
            let value = NeighborValue {
                mac: format!("aa:bb:cc:dd:{:02x}:{:02x}", (offset >> 8) & 0xff, offset & 0xff),
                state: "Reachable".to_string(),
            };
            (key, value)
        })
        .collect()
}

/// Reconcile cached and kernel state
fn reconcile_neighbors(
    cached: &HashMap<NeighborKey, NeighborValue>,
    kernel: &HashMap<NeighborKey, NeighborValue>,
) -> (Vec<NeighborKey>, Vec<NeighborKey>, Vec<NeighborKey>) {
    let mut to_add = Vec::new();
    let mut to_update = Vec::new();
    let mut to_delete = Vec::new();

    // Find additions and updates
    for (key, kernel_value) in kernel {
        match cached.get(key) {
            Some(cached_value) if cached_value != kernel_value => {
                to_update.push(key.clone());
            }
            None => {
                to_add.push(key.clone());
            }
            _ => {} // Unchanged
        }
    }

    // Find deletions (in cache but not in kernel)
    for key in cached.keys() {
        if !kernel.contains_key(key) {
            to_delete.push(key.clone());
        }
    }

    (to_add, to_update, to_delete)
}

/// Benchmark cache loading
fn bench_cache_loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_loading");

    for count in [100, 500, 1000, 5000] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let cached = load_cached_neighbors(count);
                    black_box(cached);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark reconciliation
fn bench_reconciliation(c: &mut Criterion) {
    let mut group = c.benchmark_group("reconciliation");

    for count in [100, 500, 1000, 5000] {
        group.throughput(Throughput::Elements(count as u64));

        let cached = load_cached_neighbors(count);
        let kernel = load_kernel_neighbors(count);

        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &(cached, kernel),
            |b, (cached, kernel)| {
                b.iter(|| {
                    let (adds, updates, deletes) = reconcile_neighbors(cached, kernel);
                    black_box((adds, updates, deletes));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark hash map lookups during reconciliation
fn bench_lookup_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup_performance");

    for count in [100, 1000, 10000] {
        let cached = load_cached_neighbors(count);

        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &cached,
            |b, cached| {
                // Lookup 1000 keys (some exist, some don't)
                let lookup_keys: Vec<NeighborKey> = (0..1000)
                    .map(|i| NeighborKey {
                        interface: format!("Ethernet{}", i % 64),
                        ip: format!("2001:db8::{:x}", i),
                    })
                    .collect();

                b.iter(|| {
                    let mut found = 0;
                    for key in &lookup_keys {
                        if cached.contains_key(key) {
                            found += 1;
                        }
                    }
                    black_box(found);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory usage for different cache sizes
fn bench_memory_footprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_footprint");

    for count in [100, 1000, 10000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let cached = load_cached_neighbors(count);

                    // Estimate memory usage
                    let key_size = std::mem::size_of::<NeighborKey>();
                    let value_size = std::mem::size_of::<NeighborValue>();
                    let entry_size = key_size + value_size;
                    let estimated_bytes = count * entry_size;

                    black_box((cached.len(), estimated_bytes));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark reconciliation with different overlap ratios
fn bench_reconciliation_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("reconciliation_scenarios");
    let count = 1000;

    // Scenario 1: 100% overlap (no changes)
    group.bench_function("100_percent_overlap", |b| {
        let cached = load_cached_neighbors(count);
        let kernel = cached.clone();

        b.iter(|| {
            let (adds, updates, deletes) = reconcile_neighbors(&cached, &kernel);
            black_box((adds, updates, deletes));
        });
    });

    // Scenario 2: 80% overlap (some changes)
    group.bench_function("80_percent_overlap", |b| {
        let cached = load_cached_neighbors(count);
        let kernel = load_kernel_neighbors(count);

        b.iter(|| {
            let (adds, updates, deletes) = reconcile_neighbors(&cached, &kernel);
            black_box((adds, updates, deletes));
        });
    });

    // Scenario 3: 0% overlap (complete refresh)
    group.bench_function("0_percent_overlap", |b| {
        let cached = load_cached_neighbors(count);
        let kernel = load_kernel_neighbors(count * 2); // Completely different

        b.iter(|| {
            let (adds, updates, deletes) = reconcile_neighbors(&cached, &kernel);
            black_box((adds, updates, deletes));
        });
    });

    group.finish();
}

/// Benchmark timer-based reconciliation
fn bench_timer_reconciliation(c: &mut Criterion) {
    c.bench_function("timer_reconciliation_5sec", |b| {
        let cached = load_cached_neighbors(1000);
        let mut events_during_timer = Vec::new();

        // Simulate 100 events received during 5 second timer
        for i in 0..100 {
            events_during_timer.push(NeighborKey {
                interface: format!("Ethernet{}", i % 64),
                ip: format!("2001:db8::ff{:x}", i),
            });
        }

        b.iter(|| {
            // Process queued events
            let mut processed = 0;
            for _event in &events_during_timer {
                processed += 1;
            }

            // Then reconcile with kernel
            let kernel = load_kernel_neighbors(1000);
            let (adds, updates, deletes) = reconcile_neighbors(&cached, &kernel);

            black_box((processed, adds, updates, deletes));
        });
    });
}

/// Benchmark concurrent event processing during warm restart
fn bench_concurrent_events(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_events");

    for event_count in [10, 50, 100, 500] {
        group.bench_with_input(
            BenchmarkId::from_parameter(event_count),
            &event_count,
            |b, &count| {
                let cached = load_cached_neighbors(1000);

                b.iter(|| {
                    // Simulate events arriving while warm restart is active
                    let mut event_cache = Vec::new();
                    for i in 0..count {
                        event_cache.push(NeighborKey {
                            interface: format!("Ethernet{}", i % 64),
                            ip: format!("2001:db8::new{:x}", i),
                        });
                    }

                    // After timer, merge events with kernel state
                    let kernel = load_kernel_neighbors(1000);
                    let _ = reconcile_neighbors(&cached, &kernel);

                    black_box(event_cache.len());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark interface-specific reconciliation
fn bench_interface_reconciliation(c: &mut Criterion) {
    c.bench_function("interface_specific_reconciliation", |b| {
        let cached = load_cached_neighbors(1000);
        let kernel = load_kernel_neighbors(1000);

        b.iter(|| {
            // Reconcile per interface for better parallelization
            let interfaces = [
                "Ethernet0",
                "Ethernet1",
                "Ethernet2",
                "Ethernet3",
                "Ethernet4",
            ];

            let mut total_changes = 0;
            for interface in &interfaces {
                // Filter by interface
                let cached_filtered: HashMap<_, _> = cached
                    .iter()
                    .filter(|(k, _)| k.interface.starts_with(interface))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                let kernel_filtered: HashMap<_, _> = kernel
                    .iter()
                    .filter(|(k, _)| k.interface.starts_with(interface))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                let (adds, updates, deletes) =
                    reconcile_neighbors(&cached_filtered, &kernel_filtered);
                total_changes += adds.len() + updates.len() + deletes.len();
            }

            black_box(total_changes);
        });
    });
}

criterion_group!(
    benches,
    bench_cache_loading,
    bench_reconciliation,
    bench_lookup_performance,
    bench_memory_footprint,
    bench_reconciliation_scenarios,
    bench_timer_reconciliation,
    bench_concurrent_events,
    bench_interface_reconciliation
);
criterion_main!(benches);
