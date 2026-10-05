# Testing

Pluto has a built-in test framework. No test runner to install, no assertion library to import, no special project structure. Tests live next to the code they test.

## Test Blocks

A test is a named block:

```
test "name" {
    // test body
}
```

Tests can appear anywhere in a file, alongside functions, classes, and other declarations. Multiple tests per file is normal and expected:

```
fn factorial(n: int) int {
    if n <= 1 { return 1 }
    return n * factorial(n - 1)
}

test "factorial base case" {
    expect(factorial(0)).to_equal(1)
    expect(factorial(1)).to_equal(1)
}

test "factorial recursive" {
    expect(factorial(5)).to_equal(120)
    expect(factorial(10)).to_equal(3628800)
}
```

## Running Tests

```
$ pluto test main.pluto
```

The compiler compiles the file in test mode, which generates a test runner as the entry point. Each test block runs in sequence, and the runner reports pass/fail for each.

Under normal compilation (`pluto compile` or `pluto run`), test blocks are stripped entirely. They contribute zero overhead to the production binary.

## Assertions

The assertion API is `expect(value)` followed by a matcher:

- `expect(x).to_equal(y)` -- asserts `x == y`. Works with `int`, `float`, `bool`, `string`.
- `expect(b).to_be_true()` -- asserts `b` is `true`.
- `expect(b).to_be_false()` -- asserts `b` is `false`.

A failed assertion reports the test name, expected value, and actual value, then marks the test as failed.

```
fn clamp(x: int, lo: int, hi: int) int {
    if x < lo { return lo }
    if x > hi { return hi }
    return x
}

test "clamp within range" {
    expect(clamp(5, 0, 10)).to_equal(5)
}

test "clamp below minimum" {
    expect(clamp(-3, 0, 10)).to_equal(0)
}

test "clamp above maximum" {
    expect(clamp(15, 0, 10)).to_equal(10)
}
```

## Testing Error-Raising Code

`expect_raises` asserts that a block raises an error of a specific type:

```
error InvalidInput {
    message: string
}

fn parse_positive(n: int) int {
    if n <= 0 {
        raise InvalidInput { message: "must be positive" }
    }
    return n
}

test "parse_positive succeeds" {
    let result = parse_positive(5) catch -1
    expect(result).to_equal(5)
}

test "parse_positive rejects zero" {
    expect_raises(InvalidInput) {
        parse_positive(0)!
    }
}
```

Inside the block, `!` propagates to the construct itself — the enclosing
test does not become fallible. The assertion fails if the block completes
without raising (`expected InvalidInput to be raised, but no error was
raised`) or raises a different error type (`expected InvalidInput to be
raised, got SegmentError`); either way the error is consumed by the
construct and does not escape the test. A wildcard form asserts that the
block raises *something*, whatever the type:

```
test "parse_positive rejects negatives" {
    expect_raises {
        parse_positive(-3)!
    }
}
```

The assertion is checked against the compiler's inferred error sets at
compile time. A block that cannot raise at all is a compile error
(`expect_raises block cannot raise`), and naming an error type the block
cannot produce is one too (`the block can raise 'InvalidInput' — not
'IoError'`), so an error-path test can never silently pass for the wrong
reason.

`expect_raises` asserts on the error *type* only. To inspect an error's
fields, use `catch` — it handles the error inline, exactly as it does in
production code:

```
test "error carries the message" {
    let mut seen = ""
    let v = parse_positive(0) catch err: InvalidInput {
        seen = err.message
        0
    }
    expect(v).to_equal(0)
    expect(seen).to_equal("must be positive")
}
```

## Tests in Modules

Tests defined in imported modules are private. They are not exported, not visible to the importing file, and not run when the importing file is tested. Only tests in the file passed to `pluto test` are executed.

This means each module can have its own test suite, run independently:

```
$ pluto test lib.pluto       # runs tests in lib.pluto
$ pluto test main.pluto      # runs tests in main.pluto (not lib.pluto's tests)
```

## Testing Concurrent Code

Tests that use `spawn`, channels, or `select` run under a deterministic test scheduler with four strategies — including an exhaustive mode that runs every meaningfully distinct interleaving. By default, spawned tasks run inline, so tests where tasks coordinate with each other need a `tests[scheduler: ...]` block:

```
tests[scheduler: Exhaustive] {
    test "producer and consumer" {
        // spawn + channel coordination works here
    }
}
```

See [Testing Concurrent Code](testing-concurrency.md) for the strategies, when you need each one, and how to read the scheduler's deadlock reports.
