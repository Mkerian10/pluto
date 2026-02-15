#!/usr/bin/env bash
# Manual QA script for toolchain manager (Phase 3)
# Run this once GitHub releases are available

set -e

echo "=== Toolchain Manager Manual QA ==="
echo ""

# Cleanup function
cleanup() {
    echo "Cleaning up test state..."
    rm -f ~/.pluto/active
}
trap cleanup EXIT

echo "=== Test 1: List versions (empty) ==="
./target/debug/pluto versions
echo ""

echo "=== Test 2: Use non-installed version (should error) ==="
if ./target/debug/pluto use 0.2.0 2>&1 | grep -q "not installed"; then
    echo "✓ Error message correct"
else
    echo "✗ Expected 'not installed' error"
    exit 1
fi
echo ""

echo "=== Test 3: Install version (requires GitHub release) ==="
echo "NOTE: This will fail until releases are published"
if ./target/debug/pluto install 0.1.0; then
    echo "✓ Install succeeded"

    echo "=== Test 4: List versions (should show 0.1.0) ==="
    if ./target/debug/pluto versions | grep -q "0.1.0"; then
        echo "✓ Version appears in list"
    else
        echo "✗ Version not in list"
        exit 1
    fi

    echo "=== Test 5: Use installed version ==="
    ./target/debug/pluto use 0.1.0

    echo "=== Test 6: Verify active marker ==="
    if ./target/debug/pluto versions | grep -q "* 0.1.0"; then
        echo "✓ Active version marked correctly"
    else
        echo "✗ Active version not marked"
        exit 1
    fi

    echo "=== Test 7: Check active file contents ==="
    if [ "$(cat ~/.pluto/active)" = "0.1.0" ]; then
        echo "✓ Active file contains correct version"
    else
        echo "✗ Active file has wrong content"
        exit 1
    fi

    # Note: Delegation testing requires having two different versions installed
    # and would need to be done manually
    echo ""
    echo "=== Test 8: Delegation (manual) ==="
    echo "To test delegation:"
    echo "  1. Install a different version: pluto install 0.2.0"
    echo "  2. Set it active: pluto use 0.2.0"
    echo "  3. Run: pluto --version (should show 0.2.0 via delegation)"
    echo "  4. Run: pluto compile <file> (should use 0.2.0 binary)"
else
    echo "⚠ Install failed (expected until GitHub releases are published)"
    echo "  This is normal - releases don't exist yet"
    echo ""
    echo "Once releases are available, re-run this script to verify:"
    echo "  - Install downloads binary"
    echo "  - Use sets active version"
    echo "  - Versions lists with active marker"
    echo "  - Delegation works (requires multiple versions)"
fi

echo ""
echo "=== All implemented tests passed ==="
