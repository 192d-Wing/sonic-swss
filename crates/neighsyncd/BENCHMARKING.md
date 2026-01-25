# neighsyncd Benchmarking Guide

This document describes how to use the neighsyncd benchmarking tool to measure performance optimizations.

## Building the Benchmark Tool

### Standard Build

```bash
cargo build --release -p sonic-neighsyncd --bin neighsyncd-benchmark
```

### With All Performance Features Enabled

```bash
cargo build --release -p sonic-neighsyncd --bin neighsyncd-benchmark --features perf-all
```

## Running Benchmarks

### Basic Usage

Run all benchmarks with default settings (1000 events):

```bash
./target/release/neighsyncd-benchmark
```

### Run Specific Benchmark Tests

Test netlink parsing performance:

```bash
./target/release/neighsyncd-benchmark --test netlink-parsing
```

Test Redis operation batching:

```bash
./target/release/neighsyncd-benchmark --test redis
```

Test batching efficiency:

```bash
./target/release/neighsyncd-benchmark --test batching
```

Test memory allocation patterns:

```bash
./target/release/neighsyncd-benchmark --test memory
```

### Customize Event Count

Test with 10,000 synthetic events:

```bash
./target/release/neighsyncd-benchmark --events 10000
```

### Verbose Output

Get detailed timing information:

```bash
./target/release/neighsyncd-benchmark --verbose
```

## Benchmark Tests

### 1. Netlink Parsing (`--test netlink-parsing`)

Measures the throughput of netlink message parsing.

**Metrics:**
- Events parsed per second
- Time per event (microseconds)
- Total processing time

**Performance Tip:** Enable `perf-fxhash` feature for 2-3x faster interface lookups.

**Example Output:**
```
📊 Netlink Parsing Benchmark
----------------------------
  Events parsed: 1000
  Total time: 0.05ms
  Throughput: 20000000 events/sec
  Per-event time: 0.050μs
```

### 2. Redis Operations (`--test redis`)

Compares single-operation mode vs batched (pipelined) operations.

**Metrics:**
- Time for non-batched operations
- Time for batched operations
- Speedup factor

**Expected Improvement:** 5-10x speedup with batching (P0 optimization).

**Example Output:**
```
📊 Redis Operations Benchmark
------------------------------
  Single-operation mode:
    Total time: 5.23ms
    Throughput: 191244 ops/sec
  Batched mode:
    Total time: 0.52ms
    Throughput: 1923077 ops/sec
  Speedup: 10.0x
```

### 3. Batching Efficiency (`--test batching`)

Shows round-trip reduction achieved by different batch sizes.

**Metrics:**
- Number of batches needed
- Percentage of round-trip overhead reduction

**Key Insight:** Batch size 100 achieves 99% reduction in round-trips.

**Example Output:**
```
📊 Batching Efficiency Benchmark
--------------------------------
  Batch size 1: 1000 batches, 0.0% round-trip reduction
  Batch size 10: 100 batches, 90.0% round-trip reduction
  Batch size 50: 20 batches, 98.0% round-trip reduction
  Batch size 100: 10 batches, 99.0% round-trip reduction
```

### 4. Memory Allocation (`--test memory`)

Measures memory usage patterns and allocation efficiency.

**Metrics:**
- Pre-allocated vs dynamic allocations
- Interface cache memory usage
- Allocation reduction percentage

**P3 Optimization:** Pre-allocated buffers reduce allocations by 100%.

**Example Output:**
```
📊 Memory Allocation Benchmark
------------------------------
  Pre-allocated event buffer:
    Buffer size: 1024 bytes
    Allocations for 5000 events: 1 (reused)
  Dynamic allocation (per-call):
    Allocations for 5000 events: 5000
    Reduction: 100.0%
```

## Performance Targets

Based on PERFORMANCE.md, here are the expected improvements:

| Optimization | Expected Improvement | Test Command |
|--------------|---------------------|--------------|
| Async Netlink (epoll) | 10-20% CPU reduction | System profiling |
| Redis Pipelining | 5-10x throughput | `--test redis` |
| Socket Buffer Tuning | Prevents overflow | System testing |
| Link-Local Cache | ~90% fewer queries | System profiling |
| FxHashMap | 2-3x faster lookups | `--test netlink-parsing --features perf-fxhash` |
| Batch Event Processing | 3-5x throughput | `--test batching` |
| Zero-Copy Parsing | 15-25% alloc reduction | `--test memory` |
| Pre-allocated Buffer | ~5% in tight loops | `--test memory` |

## Benchmark with Features

### Compare with/without FxHashMap

Without optimization:
```bash
cargo build --release -p sonic-neighsyncd --bin neighsyncd-benchmark
./target/release/neighsyncd-benchmark --test netlink-parsing
```

With optimization:
```bash
cargo build --release -p sonic-neighsyncd --bin neighsyncd-benchmark --features perf-fxhash
./target/release/neighsyncd-benchmark --test netlink-parsing
```

### Full Performance Profile

Build with all optimizations and run full suite:

```bash
cargo build --release -p sonic-neighsyncd --bin neighsyncd-benchmark --features perf-all
./target/release/neighsyncd-benchmark --events 10000 --verbose
```

## Integration with CI/CD

Add to your CI pipeline to track performance regressions:

```bash
#!/bin/bash
set -e

# Build with all features
cargo build --release -p sonic-neighsyncd --bin neighsyncd-benchmark --features perf-all

# Run benchmarks and capture baseline
./target/release/neighsyncd-benchmark --events 10000 > /tmp/benchmark_baseline.txt

# Expected improvements (adjust thresholds as needed)
# - Redis throughput: > 100k ops/sec
# - Netlink throughput: > 1M events/sec
# - Memory allocations: < event_count / 100

echo "✅ Performance benchmarks passed"
```

## Profiling with perf

Combine benchmarking with Linux profiling tools:

```bash
# Record CPU cycles
perf record -g ./target/release/neighsyncd-benchmark --events 50000

# View report
perf report
```

## Memory Profiling with Valgrind

Check for memory leaks and allocation patterns:

```bash
valgrind --tool=massif \
  ./target/release/neighsyncd-benchmark --events 5000 --test memory

# View results
ms_print massif.out.<pid>
```

## Notes

- Benchmarks are synthetic and may not reflect real-world conditions with actual network I/O
- Network latency dominates Redis operations; local optimization benefits are measured differently
- For realistic throughput measurements, test with actual kernel neighbor events
- Batching benefits scale with event rate (higher rate = more benefit)
