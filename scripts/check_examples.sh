#!/usr/bin/env bash
# Compile every example against the current compiler + stdlib, so examples
# can't rot silently when the language changes (mutability enforcement alone
# had broken 16 of 49 before this check existed).
set -u
BIN=${PLUTO_BIN:-target/debug/pluto}
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
fails=0
for f in examples/*/main.pt; do
  name=$(basename "$(dirname "$f")")
  case "$name" in
    # Fetches a git dependency at compile time — needs network; skipped in CI.
    git-packages) continue ;;
    # A test file (no main); exercised through the test runner instead.
    testing)
      if ! "$BIN" test "$f" > /dev/null 2>&1; then
        echo "FAIL (pluto test): $name"
        "$BIN" test "$f" 2>&1 | head -5
        fails=$((fails + 1))
      fi
      continue ;;
  esac
  if ! "$BIN" compile "$f" -o "$out/$name" --stdlib stdlib > /dev/null 2>&1; then
    echo "FAIL: $name"
    "$BIN" compile "$f" -o "$out/$name" --stdlib stdlib 2>&1 | grep -E "^error" | head -3
    fails=$((fails + 1))
  fi
done
if [ "$fails" -gt 0 ]; then
  echo "$fails example(s) failed to compile"
  exit 1
fi
echo "all examples compile"
