#!/bin/bash
#
# Performance profiling script for neighsyncd using Linux perf
#
# Usage:
#   ./profile.sh <benchmark_name> [duration_seconds]
#
# Examples:
#   ./profile.sh netlink_parsing 30
#   ./profile.sh redis_operations 60
#   ./profile.sh event_processing 30
#   ./profile.sh warm_restart 30
#
# Requirements:
#   - Linux perf tools installed
#   - Kernel with perf support
#   - CAP_PERFMON or CAP_SYS_ADMIN capability

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
BENCHMARK_NAME="${1:-netlink_parsing}"
DURATION="${2:-30}"
OUTPUT_DIR="target/profiling"
PERF_DATA="${OUTPUT_DIR}/${BENCHMARK_NAME}.perf.data"
FLAMEGRAPH="${OUTPUT_DIR}/${BENCHMARK_NAME}.svg"

# Check if perf is installed
if ! command -v perf &> /dev/null; then
    echo -e "${RED}Error: perf not found${NC}"
    echo "Install with: sudo apt-get install linux-tools-common linux-tools-generic"
    exit 1
fi

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: cargo not found${NC}"
    exit 1
fi

# Create output directory
mkdir -p "${OUTPUT_DIR}"

echo -e "${GREEN}neighsyncd Performance Profiling${NC}"
echo "=================================="
echo "Benchmark: ${BENCHMARK_NAME}"
echo "Duration: ${DURATION} seconds"
echo "Output: ${OUTPUT_DIR}"
echo ""

# Build benchmark in release mode with debug symbols
echo -e "${YELLOW}Building benchmark with debug symbols...${NC}"
RUSTFLAGS="-C force-frame-pointers=yes -C debuginfo=2" \
    cargo build --release --bench "${BENCHMARK_NAME}"

BENCHMARK_BIN="target/release/deps/${BENCHMARK_NAME}-"*
BENCHMARK_BIN=$(ls ${BENCHMARK_BIN} 2>/dev/null | head -1)

if [ ! -f "${BENCHMARK_BIN}" ]; then
    echo -e "${RED}Error: Benchmark binary not found${NC}"
    exit 1
fi

echo -e "${GREEN}Found benchmark binary: ${BENCHMARK_BIN}${NC}"
echo ""

# Run perf record
echo -e "${YELLOW}Running perf record (${DURATION}s)...${NC}"
echo "This will run the benchmark and collect performance data."
echo ""

# Check if we have permissions
if ! perf record --help &> /dev/null; then
    echo -e "${YELLOW}Note: You may need elevated privileges for perf${NC}"
    echo "Try: sudo sysctl kernel.perf_event_paranoid=-1"
    echo "Or run with: sudo ./profile.sh ${BENCHMARK_NAME} ${DURATION}"
    echo ""
fi

# Record with detailed options
perf record \
    --call-graph dwarf \
    --freq=997 \
    --output="${PERF_DATA}" \
    --timeout="${DURATION}000" \
    "${BENCHMARK_BIN}" --bench 2>&1 | head -50 &

PERF_PID=$!

# Show progress
for i in $(seq 1 ${DURATION}); do
    echo -n "."
    sleep 1
done
echo ""

wait ${PERF_PID} 2>/dev/null || true

if [ ! -f "${PERF_DATA}" ]; then
    echo -e "${RED}Error: Profiling data not generated${NC}"
    exit 1
fi

echo -e "${GREEN}Profiling data collected: ${PERF_DATA}${NC}"
echo ""

# Generate perf report
echo -e "${YELLOW}Generating perf report...${NC}"
perf report --input="${PERF_DATA}" --stdio > "${OUTPUT_DIR}/${BENCHMARK_NAME}.report.txt" 2>&1 || true
echo -e "${GREEN}Report saved: ${OUTPUT_DIR}/${BENCHMARK_NAME}.report.txt${NC}"
echo ""

# Show top functions
echo -e "${YELLOW}Top 20 functions by CPU time:${NC}"
perf report --input="${PERF_DATA}" --stdio --sort=dso,symbol | head -40
echo ""

# Generate annotated source if available
echo -e "${YELLOW}Generating annotated source...${NC}"
perf annotate --input="${PERF_DATA}" --stdio > "${OUTPUT_DIR}/${BENCHMARK_NAME}.annotated.txt" 2>&1 || true

# Try to generate flamegraph if available
if command -v flamegraph &> /dev/null || command -v inferno-flamegraph &> /dev/null; then
    echo -e "${YELLOW}Generating flamegraph...${NC}"

    if command -v inferno-flamegraph &> /dev/null; then
        # Using inferno (Rust implementation)
        perf script --input="${PERF_DATA}" | inferno-collapse-perf | inferno-flamegraph > "${FLAMEGRAPH}" 2>/dev/null || true
    elif command -v flamegraph &> /dev/null; then
        # Using flamegraph.pl
        perf script --input="${PERF_DATA}" > "${OUTPUT_DIR}/${BENCHMARK_NAME}.perf.script"
        flamegraph "${OUTPUT_DIR}/${BENCHMARK_NAME}.perf.script" > "${FLAMEGRAPH}" 2>/dev/null || true
    fi

    if [ -f "${FLAMEGRAPH}" ]; then
        echo -e "${GREEN}Flamegraph saved: ${FLAMEGRAPH}${NC}"
        echo "Open in browser: file://$(pwd)/${FLAMEGRAPH}"
    fi
else
    echo -e "${YELLOW}Note: Install cargo-flamegraph for flamegraph generation${NC}"
    echo "  cargo install inferno"
fi

echo ""
echo -e "${GREEN}Profiling complete!${NC}"
echo ""
echo "Results:"
echo "  - Perf data: ${PERF_DATA}"
echo "  - Report: ${OUTPUT_DIR}/${BENCHMARK_NAME}.report.txt"
echo "  - Annotated: ${OUTPUT_DIR}/${BENCHMARK_NAME}.annotated.txt"
if [ -f "${FLAMEGRAPH}" ]; then
    echo "  - Flamegraph: ${FLAMEGRAPH}"
fi
echo ""

# Summary statistics
echo -e "${YELLOW}Performance Summary:${NC}"
perf stat --input="${PERF_DATA}" 2>&1 || true

echo ""
echo "To view detailed report:"
echo "  perf report --input=${PERF_DATA}"
echo ""
echo "To annotate specific function:"
echo "  perf annotate --input=${PERF_DATA} <function_name>"
echo ""
