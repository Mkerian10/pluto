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
    // runs forever until the process is killed
}
```

## for / in Loops

`for` iterates over arrays:

```
fn main() {
    let names = ["alice", "bob", "charlie"]
    for name in names {
        print("hello, {name}")
    }
}
```

The loop variable's type is inferred from the array element type. You can nest loops:

```
fn main() {
    let rows = [1, 2]
    let cols = [10, 20]
    for r in rows {
        for c in cols {
            print(r + c)
        }
    }
}
```

The loop variable doesn't leak into the outer scope:

```
fn main() {
    let x = 999
    for x in [1, 2, 3] {
        print(x)        // 1, 2, 3
    }
    print(x)            // 999
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
    let i = 1
    while i <= 15 {
        if i % 15 == 0 {
            print("FizzBuzz")
        } else if i % 3 == 0 {
            print("Fizz")
        } else if i % 5 == 0 {
            print("Buzz")
        } else {
            print(i)
        }
        i = i + 1
    }
}
```
