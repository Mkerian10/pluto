#!/usr/bin/env bash
# Concurrency benchmark runner with variance reporting.
#
# Unlike benchmarks/run_benchmarks.sh (single run per benchmark), this runs
# each workload R times and reports min / median / mean / stddev, because
# concurrency numbers are noisy and a single sample prunes on phantoms.
#
# Usage:
#   benchmarks/concurrency/run.sh [--runs N] [--json out.json] [workload ...]
# With no workload names, runs the whole suite.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

RUNS=7
JSON_OUTPUT=""
WORKLOADS=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --runs) RUNS="$2"; shift 2 ;;
        --json) JSON_OUTPUT="$2"; shift 2 ;;
        -*) echo "Unknown option: $1" >&2; exit 1 ;;
        *) WORKLOADS+=("$1"); shift ;;
    esac
done

if [ ${#WORKLOADS[@]} -eq 0 ]; then
    WORKLOADS=(scalar_baseline lock_class_single lock_entity_single spawn_join chan_throughput entity_uncontended entity_contended spawn_pingpong green_pingpong)
fi

echo "Building compiler (release)..."
cargo build --release --manifest-path "$PROJECT_DIR/Cargo.toml" 2>&1 | tail -1
PLUTO="$PROJECT_DIR/target/release/pluto"
echo ""
echo "Runs per workload: $RUNS"
printf '%-22s %8s %8s %8s %8s\n' "workload" "min" "med" "mean" "stddev"
printf '%-22s %8s %8s %8s %8s\n' "--------" "---" "---" "----" "------"

JSON_ENTRIES=""
for name in "${WORKLOADS[@]}"; do
    src="$SCRIPT_DIR/${name}.pt"
    if [ ! -f "$src" ]; then
        printf '%-22s %s\n' "$name" "SKIP (not found)"
        continue
    fi
    # Isolate: sibling .pt files in one dir auto-merge.
    wdir="$TMP_DIR/$name"; mkdir -p "$wdir"; cp "$src" "$wdir/"
    bin="$TMP_DIR/$name.bin"
    if ! "$PLUTO" compile "$wdir/${name}.pt" -o "$bin" 2>"$TMP_DIR/$name.err"; then
        printf '%-22s %s\n' "$name" "FAIL (compile)"
        sed 's/^/      /' "$TMP_DIR/$name.err" | head -6
        continue
    fi

    samples=()
    for ((r=0; r<RUNS; r++)); do
        out=$("$bin" 2>&1) || true
        ms=$(echo "$out" | sed -n 's/^elapsed: \([0-9]*\) ms/\1/p')
        if [ -z "$ms" ]; then
            printf '%-22s %s\n' "$name" "FAIL (no timing): $out"
            continue 2
        fi
        samples+=("$ms")
    done

    stats=$(printf '%s\n' "${samples[@]}" | sort -n | awk '
        { a[NR]=$1; sum+=$1 }
        END {
            n=NR; min=a[1];
            med=(n%2)? a[int(n/2)+1] : (a[n/2]+a[n/2+1])/2;
            mean=sum/n;
            for(i=1;i<=n;i++){d=a[i]-mean; ss+=d*d}
            sd=(n>1)? sqrt(ss/(n-1)) : 0;
            printf "%d %g %.1f %.1f", min, med, mean, sd;
        }')
    read -r smin smed smean ssd <<< "$stats"
    printf '%-22s %8s %8s %8s %8s\n' "$name" "$smin" "$smed" "$smean" "$ssd"
    entry="{\"name\":\"$name\",\"unit\":\"ms\",\"min\":$smin,\"median\":$smed,\"mean\":$smean,\"stddev\":$ssd,\"runs\":$RUNS}"
    JSON_ENTRIES="${JSON_ENTRIES:+$JSON_ENTRIES,}$entry"
done

if [ -n "$JSON_OUTPUT" ]; then
    echo "[$JSON_ENTRIES]" > "$JSON_OUTPUT"
    echo ""
    echo "JSON written to $JSON_OUTPUT"
fi
