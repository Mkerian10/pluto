# Testing

Pluto has a built-in test framework. Write tests directly in your source files using `test` blocks, and run them with `plutoc test`.

## Writing Tests

A test is a named block that contains assertions:

```
fn add(a: int, b: int) int {
    return a + b
}

test "addition works" {
    expect(add(1, 2)).to_equal(3)
    expect(add(-1, 1)).to_equal(0)
    expect(add(0, 0)).to_equal(0)
}
```

Test names must be unique within a file.

## Running Tests

Use the `test` subcommand:

```bash
plutoc test my_file.pluto
```

Output shows each test result:

```
Running 3 tests...
  PASS  addition works
  PASS  negative addition
  PASS  string equality
3/3 passed
```

If a test fails, the output shows the expected and actual values with the file and line number.

## Assertions

All assertions start with `expect(value)` followed by a method:

### `to_equal(expected)`

Checks that two values are equal. Works with `int`, `float`, `bool`, and `string`:

```
test "equality" {
    expect(2 + 2).to_equal(4)
    expect("hello").to_equal("hello")
    expect(3.14).to_equal(3.14)
}
```

### `to_be_true()`

Checks that a boolean value is `true`:

```
test "boolean true" {
    expect(5 > 3).to_be_true()
    expect(true).to_be_true()
}
```

### `to_be_false()`

Checks that a boolean value is `false`:

```
test "boolean false" {
    expect(1 > 100).to_be_false()
    expect(false).to_be_false()
}
```

## Tests and Regular Code

Tests live alongside regular functions and classes. When you compile normally (`plutoc compile` or `plutoc run`), test blocks are stripped out -- they don't affect your program:

```
fn double(x: int) int {
    return x * 2
}

// Only runs with `plutoc test`, ignored by `plutoc run`
test "double works" {
    expect(double(5)).to_equal(10)
}

fn main() {
    print(double(21))
}
```

`plutoc run` prints `42`. `plutoc test` runs the test.

## Multiple Test Blocks

You can have as many test blocks as you want in a file:

```
test "math basics" {
    expect(1 + 1).to_equal(2)
}

test "string basics" {
    expect("a" + "b").to_equal("ab")
}

test "boolean basics" {
    expect(true).to_be_true()
    expect(false).to_be_false()
}
```

## Tests in Modules

When you import a module, its tests are **not** included. Tests are private to the file they're defined in. This means importing a well-tested module doesn't pollute your test run with its internal tests.
