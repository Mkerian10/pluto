#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

usage() {
    echo "Usage: $0 [--arm64|--amd64|--no-cache|--build-only|-h]"
    echo ""
    echo "  --arm64       Test only on linux/arm64 (native on Apple Silicon)"
    echo "  --amd64       Test only on linux/amd64 (QEMU on Apple Silicon)"
    echo "  --no-cache    Clean rebuild (also clears cargo cache volumes)"
    echo "  --build-only  Build images without running tests"
    echo "  -h, --help    Show this help"
}

SERVICES=""
NO_CACHE=""
BUILD_ONLY=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --arm64)  SERVICES="test-arm64"; shift ;;
        --amd64)  SERVICES="test-amd64"; shift ;;
        --no-cache) NO_CACHE="--no-cache"; shift ;;
        --build-only) BUILD_ONLY=true; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown: $1"; usage; exit 1 ;;
    esac
done

echo "=== Pluto Docker Test Runner ==="

# Clean volumes if --no-cache
if [[ -n "$NO_CACHE" ]]; then
    docker compose down -v 2>/dev/null || true
fi

# Build
docker compose build $NO_CACHE $SERVICES
if $BUILD_ONLY; then echo "Build complete."; exit 0; fi

# Run tests
if [[ -z "$SERVICES" ]]; then
    # Both architectures — run sequentially for clean output
    FAILED=0
    for svc in test-arm64 test-amd64; do
        echo ""
        docker compose up --exit-code-from "$svc" "$svc" || FAILED=1
    done
    docker compose down
    [[ $FAILED -eq 0 ]] && echo "=== ALL PASSED ===" || { echo "=== SOME FAILED ==="; exit 1; }
else
    docker compose up --abort-on-container-exit --exit-code-from "$SERVICES" "$SERVICES"
    EXIT=$?
    docker compose down
    exit $EXIT
fi
