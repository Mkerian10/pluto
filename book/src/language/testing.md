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
$ plutoc test main.pluto
```

The compiler compiles the file in test mode, which generates a test runner as the entry point. Each test block runs in sequence, and the runner reports pass/fail for each.

Under normal compilation (`plutoc compile` or `plutoc run`), test blocks are stripped entirely. They contribute zero overhead to the production binary.

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

Use `catch` to test functions that raise errors:

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
    let result = parse_positive(0) catch -1
    expect(result).to_equal(-1)
}
```

The `catch` expression handles the error inline, exactly as it does in production code. There is no special test-only error handling mechanism.

## Tests in Modules

Tests defined in imported modules are private. They are not exported, not visible to the importing file, and not run when the importing file is tested. Only tests in the file passed to `plutoc test` are executed.

This means each module can have its own test suite, run independently:

```
$ plutoc test lib.pluto       # runs tests in lib.pluto
$ plutoc test main.pluto      # runs tests in main.pluto (not lib.pluto's tests)
```
