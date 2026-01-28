#!/bin/bash
# RuVector Memory Optimizer - macOS Test Script
# Run this on Mac Mini to build and test the macOS port
#
# Usage:
#   From Windows: ssh cohen@100.123.117.38 "cd ~/workspace/ruvector-memopt && ./scripts/test-mac.sh"
#   On Mac: ./scripts/test-mac.sh

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_header() { echo -e "\n${CYAN}═══════════════════════════════════════════════════════════${NC}"; echo -e "${CYAN}  $1${NC}"; echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}\n"; }

# Check if running on macOS
if [[ "$(uname)" != "Darwin" ]]; then
    log_error "This script must be run on macOS"
    exit 1
fi

log_header "RuVector Memory Optimizer - macOS Test Suite"

# System info
echo "System: $(uname -s) $(uname -r)"
echo "Arch: $(uname -m)"
echo "Hostname: $(hostname)"
if [[ "$(uname -m)" == "arm64" ]]; then
    echo "Chip: Apple Silicon"
else
    echo "Chip: Intel"
fi
echo ""

# Check Rust installation
log_info "Checking Rust installation..."
if ! command -v cargo &> /dev/null; then
    log_warn "Rust not found. Installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
log_success "Rust $(rustc --version | cut -d' ' -f2)"

# Build
log_header "Building RuVector (Release)"
cargo build --release 2>&1 | tail -20

if [[ ! -f "target/release/ruvector-memopt" ]]; then
    log_error "Build failed - binary not found"
    exit 1
fi
log_success "Build complete"

# Binary info
log_header "Binary Information"
ls -lh target/release/ruvector-memopt
file target/release/ruvector-memopt

# Run tests
log_header "Running Tests"

echo "1. Memory Status:"
echo "─────────────────"
./target/release/ruvector-memopt status
echo ""

echo "2. CPU Capabilities:"
echo "────────────────────"
./target/release/ruvector-memopt cpu
echo ""

echo "3. Configuration:"
echo "─────────────────"
./target/release/ruvector-memopt config
echo ""

echo "4. Process PageRank (top 5):"
echo "────────────────────────────"
./target/release/ruvector-memopt pagerank --top 5
echo ""

echo "5. Process Clusters:"
echo "────────────────────"
./target/release/ruvector-memopt clusters --max 3
echo ""

# Benchmark
log_header "Running Benchmarks"
echo "Basic benchmarks (100 iterations):"
./target/release/ruvector-memopt bench --iterations 100

echo ""
echo "Advanced algorithm benchmarks:"
./target/release/ruvector-memopt bench --advanced --iterations 50

# Memory optimization test (dry-run)
log_header "Testing Optimization (Dry Run)"
./target/release/ruvector-memopt optimize --dry-run

# Real optimization (if running with sudo)
if [[ $(id -u) -eq 0 ]]; then
    log_header "Testing Real Optimization (with sudo)"
    ./target/release/ruvector-memopt optimize
else
    log_warn "Not running as root - skipping real optimization test"
    echo "To test full optimization: sudo ./target/release/ruvector-memopt optimize"
fi

# Tray test info
log_header "Menu Bar Tray"
echo "To test the menu bar tray application:"
echo "  ./target/release/ruvector-memopt tray"
echo ""
echo "Note: The tray app will appear in the menu bar and provide:"
echo "  - Real-time memory status"
echo "  - Auto-optimization"
echo "  - Manual optimization controls"
echo "  - Threshold settings"

# Summary
log_header "Test Summary"
log_success "All basic tests passed!"
echo ""
echo "Available commands:"
echo "  ruvector-memopt status      - Show memory status"
echo "  ruvector-memopt optimize    - Run optimization"
echo "  ruvector-memopt daemon      - Run background daemon"
echo "  ruvector-memopt tray        - Start menu bar app"
echo "  ruvector-memopt bench       - Run benchmarks"
echo "  ruvector-memopt pagerank    - Process priority analysis"
echo "  ruvector-memopt clusters    - Process clustering"
echo "  ruvector-memopt patterns    - Memory pattern analysis"
echo "  ruvector-memopt cpu         - CPU/SIMD capabilities"
echo "  ruvector-memopt dashboard   - Real-time dashboard"
echo ""
echo "For full optimization (purge), run with sudo:"
echo "  sudo ./target/release/ruvector-memopt optimize --aggressive"
echo ""

# Install globally (optional)
read -p "Install ruvector-memopt to /usr/local/bin? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    sudo cp target/release/ruvector-memopt /usr/local/bin/
    log_success "Installed to /usr/local/bin/ruvector-memopt"
fi

log_header "Done!"
