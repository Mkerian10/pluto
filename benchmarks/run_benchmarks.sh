#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BENCH_DIR="$SCRIPT_DIR"
TMP_DIR=$(mktemp -d)

trap 'rm -rf "$TMP_DIR"' EXIT

echo "=== Pluto Runtime Benchmarks ==="
echo ""

# Build compiler in release mode
echo "Building compiler (release)..."
cargo build --release --manifest-path "$PROJECT_DIR/Cargo.toml" 2>&1 | tail -1
PLUTOC="$PROJECT_DIR/target/release/plutoc"
echo ""

BENCHMARKS=(
    fib
    loop_sum
    string_concat
    array_push
    array_iter
    class_method
    closure_call
    trait_dispatch
    gc_churn
    gc_binary_trees
    gc_string_pressure
    sieve
    bounce
    towers
    permute
    queens
    fannkuch_redux
    spectral_norm
    nbody
    mandelbrot
    monte_carlo
    storage
    list
    fft
    sor
)

PASS=0
FAIL=0

for bench in "${BENCHMARKS[@]}"; do
    src="$BENCH_DIR/${bench}.pluto"
    bin="$TMP_DIR/${bench}"

    if [ ! -f "$src" ]; then
        echo "SKIP  $bench (file not found)"
        continue
    fi

    # Copy source to isolated temp dir (Pluto merges sibling .pluto files)
    bench_dir="$TMP_DIR/${bench}_dir"
    mkdir -p "$bench_dir"
    cp "$src" "$bench_dir/"

    # Compile
    if ! "$PLUTOC" compile "$bench_dir/${bench}.pluto" -o "$bin" 2>/dev/null; then
        echo "FAIL  $bench (compilation error)"
        FAIL=$((FAIL + 1))
        continue
    fi

    # Run
    output=$("$bin" 2>&1) || true
    elapsed_line=$(echo "$output" | grep "^elapsed:" || echo "")

    if [ -n "$elapsed_line" ]; then
        echo "OK    $bench  $elapsed_line"
        PASS=$((PASS + 1))
    else
        echo "FAIL  $bench (no timing output)"
        echo "      output: $output"
        FAIL=$((FAIL + 1))
    fi
done

echo ""
echo "--- Results: $PASS passed, $FAIL failed ---"
exit $FAIL
