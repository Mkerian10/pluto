# MCP add_declaration Drops Imports and Test Blocks

**Project:** Cassini
**Date:** 2026-02-10
**Tool(s):** add_declaration
**Type:** bug

## What Happened

Used `add_declaration` to create a new test file (`cassini/tests/metrics/metrics_test.pluto`). The source string included an `import src` at the top and 8 `test "..." { ... }` blocks.

The tool stripped the `import src` line entirely and converted all `test "name" { ... }` blocks into plain `fn __test_0() { ... }`, `fn __test_1() { ... }`, etc. The resulting file had no import and no test declarations — just regular functions that the test runner wouldn't recognize.

Input (abbreviated):
```pluto
import src

test "create_cassini_metrics returns valid handles" {
    let m = src.create_cassini_metrics("127.0.0.1", 9090)
    expect(m.requests_total.name).to_equal("proxy_requests_total")
}
```

Output file started with:
```pluto
fn __test_0() {
    let m = src.create_cassini_metrics("127.0.0.1", 9090)
```

## Assessment

`add_declaration` only handles top-level declarations (functions, classes, enums, etc.) — it silently drops imports and doesn't understand `test` as a declaration kind. This means it can't be used to create test files, which are a very common workflow.

Had to fall back to the Write tool (raw file creation) to get a working test file.

## Suggestion

1. Handle `import` statements in `add_declaration` — either add them to the file's import list or return an error explaining they need to be added separately.
2. Support `test` blocks as a declaration kind, or document that they're not supported and suggest using Write instead.
