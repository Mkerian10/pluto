# Arrays

## Creating Arrays

Arrays are created with square brackets:

```
let nums = [1, 2, 3]
let names = ["alice", "bob"]
let empty: [int] = []
```

All elements must be the same type.

## Indexing

Access elements by zero-based index:

```
let a = [10, 20, 30]
print(a[0])     // 10
print(a[2])     // 30
```

You can also assign to an index:

```
let a = [10, 20, 30]
a[1] = 99
print(a[1])     // 99
```

## Length and Push

```
let a = [1, 2, 3]
print(a.len())      // 3

a.push(4)
print(a.len())      // 4
print(a[3])         // 4
```

## Iterating

Use `for`/`in` to loop over array elements:

```
fn main() {
    let fruits = ["apple", "banana", "cherry"]
    for fruit in fruits {
        print(fruit)
    }
}
```

## Arrays as Parameters

Arrays can be passed to and returned from functions:

```
fn sum(nums: [int]) int {
    let total = 0
    for n in nums {
        total = total + n
    }
    return total
}

fn main() {
    print(sum([1, 2, 3, 4]))   // 10
}
```

## Nested Arrays

Arrays can contain other arrays:

```
let grid = [[1, 2], [3, 4]]
print(grid[0][1])    // 2
```

## Array Type Syntax

The type of an array of `int` is written `[int]`. For function signatures:

```
fn first(a: [int]) int {
    return a[0]
}

fn make_pair(x: int, y: int) [int] {
    let result = [x, y]
    return result
}
```
