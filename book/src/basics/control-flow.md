# Control Flow

## if / else

```
fn main() {
    let x = 10

    if x > 5 {
        print("big")
    }

    if x > 100 {
        print("huge")
    } else {
        print("not huge")
    }
}
```

You can chain `else if`:

```
fn classify(x: int) {
    if x > 0 {
        print("positive")
    } else if x < 0 {
        print("negative")
    } else {
        print("zero")
    }
}
```

## while Loops

```
fn main() {
    let i = 0
    while i < 5 {
        print(i)
        i = i + 1
    }
}
```

For infinite loops, use `while true`:

```
while true {
    // runs forever until break
}
```

## for / in Loops

`for` iterates over arrays, ranges, and strings:

```
fn main() {
    let names = ["alice", "bob", "charlie"]
    for name in names {
        print("hello, {name}")
    }
}
```

### Ranges

You can also iterate over integer ranges:

```
fn main() {
    // exclusive: 0, 1, 2, 3, 4
    for i in 0..5 {
        print(i)
    }

    // inclusive: 0, 1, 2, 3, 4, 5
    for i in 0..=5 {
        print(i)
    }
}
```

### Strings

You can iterate over the characters of a string:

```
fn main() {
    for c in "hello" {
        print(c)    // "h", "e", "l", "l", "o"
    }
}
```

Each element is a single-character `string`.

### Range endpoints

Range endpoints can be any integer expression:

```
fn main() {
    let n = 10
    for i in 0..(n * 2) {
        print(i)
    }
}
```

If the start is greater than or equal to the end (for exclusive ranges) or greater than the end (for inclusive ranges), the loop body doesn't execute:

```
fn main() {
    for i in 5..3 {
        print(i)  // never runs
    }
}
```

### Nesting and scoping

You can nest loops. The loop variable's type is inferred from the array element type (or `int` for ranges). The loop variable doesn't leak into the outer scope:

```
fn main() {
    let x = 999
    for x in [1, 2, 3] {
        print(x)        // 1, 2, 3
    }
    print(x)            // 999
}
```

## break and continue

Use `break` to exit a loop early, and `continue` to skip to the next iteration:

```
fn main() {
    // break exits the loop
    for i in 0..100 {
        if i == 5 {
            break
        }
        print(i)  // 0, 1, 2, 3, 4
    }

    // continue skips the rest of the body
    for i in 0..6 {
        if i % 2 == 0 {
            continue
        }
        print(i)  // 1, 3, 5
    }
}
```

Both work in `while` loops too:

```
fn main() {
    let i = 0
    while true {
        if i == 3 {
            break
        }
        print(i)
        i = i + 1
    }
}
```

In nested loops, `break` and `continue` affect only the innermost loop:

```
fn main() {
    for i in 0..3 {
        for j in 0..10 {
            if j == 2 {
                break  // only breaks inner loop
            }
            print(i * 10 + j)
        }
    }
    // prints: 0, 1, 10, 11, 20, 21
}
```

`break` and `continue` cannot be used inside closures to affect an enclosing loop — this is a compile error:

```
while true {
    let f = () => { break }  // ERROR: break can only be used inside a loop
}
```

## Early Return

You can use `return` to exit a function from inside a loop:

```
fn find_first_positive(nums: [int]) int {
    for n in nums {
        if n > 0 {
            return n
        }
    }
    return 0
}
```

## FizzBuzz

Putting it all together:

```
fn main() {
    for i in 1..=15 {
        if i % 15 == 0 {
            print("FizzBuzz")
        } else if i % 3 == 0 {
            print("Fizz")
        } else if i % 5 == 0 {
            print("Buzz")
        } else {
            print(i)
        }
    }
}
```
