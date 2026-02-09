#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REF_DIR="$SCRIPT_DIR/reference"
TMP_DIR=$(mktemp -d)
RESULTS="$TMP_DIR/results.txt"

trap 'rm -rf "$TMP_DIR"' EXIT

BENCHMARKS=(fib loop_sum sieve bounce towers permute queens)

extract_ms() {
    echo "$1" | grep "^elapsed:" | sed 's/elapsed: \([0-9]*\) ms/\1/' || echo "-"
}

# Store result: lang bench ms
store() {
    echo "$1 $2 $3" >> "$RESULTS"
}

# Lookup result
lookup() {
    local lang="$1" bench="$2"
    local val
    val=$(grep "^${lang} ${bench} " "$RESULTS" 2>/dev/null | awk '{print $3}') || true
    echo "${val:--}"
}

echo "=== Cross-Language Benchmark Comparison ==="
echo ""

# --- Build Pluto compiler ---
echo "Building Pluto compiler (release)..."
cargo build --release --manifest-path "$PROJECT_DIR/Cargo.toml" 2>&1 | tail -1
PLUTOC="$PROJECT_DIR/target/release/plutoc"
echo ""

# --- Compile C benchmarks ---
echo "Compiling C benchmarks (-O2)..."
for bench in "${BENCHMARKS[@]}"; do
    cc -O2 -o "$TMP_DIR/c_${bench}" "$REF_DIR/c/${bench}.c" 2>/dev/null || echo "  FAIL: c/$bench"
done
echo ""

# --- Run all benchmarks ---
echo "Running benchmarks..."
touch "$RESULTS"

for bench in "${BENCHMARKS[@]}"; do
    echo -n "  $bench: "

    # Pluto
    src="$SCRIPT_DIR/${bench}.pluto"
    bin="$TMP_DIR/pluto_${bench}"
    bench_dir="$TMP_DIR/${bench}_dir"
    mkdir -p "$bench_dir"
    cp "$src" "$bench_dir/"
    if "$PLUTOC" compile "$bench_dir/${bench}.pluto" -o "$bin" 2>/dev/null; then
        out=$("$bin" 2>&1) || true
        store pluto "$bench" "$(extract_ms "$out")"
    else
        store pluto "$bench" "-"
    fi
    echo -n "pluto "

    # C -O2
    c_bin="$TMP_DIR/c_${bench}"
    if [ -f "$c_bin" ]; then
        out=$("$c_bin" 2>&1) || true
        store c "$bench" "$(extract_ms "$out")"
    else
        store c "$bench" "-"
    fi
    echo -n "c "

    # Go
    go_src="$REF_DIR/go/${bench}.go"
    if [ -f "$go_src" ]; then
        go_bin="$TMP_DIR/go_${bench}"
        if go build -o "$go_bin" "$go_src" 2>/dev/null; then
            out=$("$go_bin" 2>&1) || true
            store go "$bench" "$(extract_ms "$out")"
        else
            store go "$bench" "-"
        fi
    else
        store go "$bench" "-"
    fi
    echo -n "go "

    # Python
    py_src="$REF_DIR/python/${bench}.py"
    if [ -f "$py_src" ]; then
        out=$(python3 "$py_src" 2>&1) || true
        store python "$bench" "$(extract_ms "$out")"
    else
        store python "$bench" "-"
    fi
    echo "python"
done

# --- Print comparison table ---
echo ""
echo ""
printf "%-12s %10s %10s %10s %10s\n" "Benchmark" "Pluto" "C -O2" "Go" "Python"
printf "%-12s %10s %10s %10s %10s\n" "---------" "-----" "-----" "--" "------"

for bench in "${BENCHMARKS[@]}"; do
    pluto=$(lookup pluto "$bench")
    c=$(lookup c "$bench")
    go_val=$(lookup go "$bench")
    py=$(lookup python "$bench")

    # Format with "ms" suffix
    [ "$pluto" != "-" ] && pluto="${pluto} ms"
    [ "$c" != "-" ] && c="${c} ms"
    [ "$go_val" != "-" ] && go_val="${go_val} ms"
    [ "$py" != "-" ] && py="${py} ms"

    printf "%-12s %10s %10s %10s %10s\n" "$bench" "$pluto" "$c" "$go_val" "$py"
done

echo ""

# --- Print ratios (Pluto / other) ---
echo "Ratios (Pluto / Language — lower is better for Pluto):"
printf "%-12s %10s %10s %10s\n" "Benchmark" "vs C" "vs Go" "vs Python"
printf "%-12s %10s %10s %10s\n" "---------" "----" "-----" "---------"

for bench in "${BENCHMARKS[@]}"; do
    pluto=$(lookup pluto "$bench")
    c=$(lookup c "$bench")
    go_val=$(lookup go "$bench")
    py=$(lookup python "$bench")

    if [ "$pluto" != "-" ] && [ "$c" != "-" ] && [ "$c" != "0" ]; then
        ratio_c=$(awk "BEGIN {printf \"%.1fx\", $pluto / $c}")
    else
        ratio_c="-"
    fi

    if [ "$pluto" != "-" ] && [ "$go_val" != "-" ] && [ "$go_val" != "0" ]; then
        ratio_go=$(awk "BEGIN {printf \"%.1fx\", $pluto / $go_val}")
    else
        ratio_go="-"
    fi

    if [ "$pluto" != "-" ] && [ "$py" != "-" ] && [ "$py" != "0" ]; then
        ratio_py=$(awk "BEGIN {printf \"%.1fx\", $pluto / $py}")
    else
        ratio_py="-"
    fi

    printf "%-12s %10s %10s %10s\n" "$bench" "$ratio_c" "$ratio_go" "$ratio_py"
done

echo ""
echo "Done."
