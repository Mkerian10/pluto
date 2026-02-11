# std.io

Console I/O: printing without newlines and reading from stdin.

```
import std.io
```

## Functions

### print / println

```
io.print(s: string)      // write to stdout, no trailing newline
io.println(s: string)    // write to stdout with trailing newline
```

Note: The built-in `print()` (no import needed) already prints with a newline. `io.print()` is useful when you need to write a prompt on the same line as user input.

### read_line

```
io.read_line() string
```

Reads one line from stdin (blocking). Returns the input without the trailing newline.

## Example: Interactive Prompt

```
import std.io

fn main() int? {
    io.print("What is your name? ")
    let name = io.read_line()
    print("Hello, {name}!")

    io.print("Enter a number: ")
    let n = io.read_line().to_int()?
    print("Doubled: {n * 2}")

    return none
}
```

`to_int()` returns `int?` -- it yields `none` if the string is not a valid integer. The `?` operator propagates the `none` value, causing the function to return early.
