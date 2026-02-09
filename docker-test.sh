#!/usr/bin/env bash
set -euo pipefail

PLATFORMS=("linux/arm64" "linux/amd64")

if [[ "${1:-}" == "--platform" ]]; then
    PLATFORMS=("$2")
    shift 2
fi

for platform in "${PLATFORMS[@]}"; do
    echo "=== Testing on $platform ==="
    docker build --platform "$platform" -t "plutoc-test-${platform##*/}" .
    docker run --rm --platform "$platform" "plutoc-test-${platform##*/}" cargo test "$@"
done
