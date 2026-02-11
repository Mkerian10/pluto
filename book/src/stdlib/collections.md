# std.collections

Functional operations on arrays. All higher-order functions take closures as arguments.

```
import std.collections
```

## Types

### Pair\<A, B\>

A generic two-element tuple used by `zip` and `enumerate`.

```
pub class Pair<A, B> {
    first: A
    second: B
}
```

## Transforms

### map

```
collections.map<T, U>(arr: [T], f: fn(T) U) [U]
```

Applies `f` to each element, returns a new array of results.

```
let doubled = collections.map([1, 2, 3], (x: int) => x * 2)
// [2, 4, 6]
```

### flat_map

```
collections.flat_map<T, U>(arr: [T], f: fn(T) [U]) [U]
```

Maps each element to an array, then flattens the result.

```
let expanded = collections.flat_map([1, 2, 3], (x: int) => [x, x * 10])
// [1, 10, 2, 20, 3, 30]
```

### filter

```
collections.filter<T>(arr: [T], f: fn(T) bool) [T]
```

Returns elements for which `f` returns `true`.

```
let evens = collections.filter([1, 2, 3, 4], (x: int) => x % 2 == 0)
// [2, 4]
```

## Reductions

### fold

```
collections.fold<T, U>(arr: [T], initial: U, f: fn(U, T) U) U
```

Left fold with an initial accumulator value.

```
let sum = collections.fold([1, 2, 3], 0, (acc: int, x: int) => acc + x)
// 6
```

### reduce

```
collections.reduce<T>(arr: [T], f: fn(T, T) T) T
```

Like `fold`, but uses the first element as the initial value. Array must be non-empty.

```
let product = collections.reduce([2, 3, 4], (a: int, b: int) => a * b)
// 24
```

### sum / sum_float

```
collections.sum(arr: [int]) int
collections.sum_float(arr: [float]) float
```

```
collections.sum([10, 20, 30])          // 60
collections.sum_float([1.5, 2.5])      // 4.0
```

## Predicates

### any / all

```
collections.any<T>(arr: [T], f: fn(T) bool) bool
collections.all<T>(arr: [T], f: fn(T) bool) bool
```

```
collections.any([1, 2, 3], (x: int) => x > 2)    // true
collections.all([1, 2, 3], (x: int) => x > 0)     // true
```

### count

```
collections.count<T>(arr: [T], f: fn(T) bool) int
```

Returns the number of elements satisfying `f`.

```
collections.count([1, 2, 3, 4, 5], (x: int) => x > 3)    // 2
```

## Slicing and Reordering

### reverse / take / drop

```
collections.reverse<T>(arr: [T]) [T]
collections.take<T>(arr: [T], n: int) [T]
collections.drop<T>(arr: [T], n: int) [T]
```

```
collections.reverse([1, 2, 3])     // [3, 2, 1]
collections.take([1, 2, 3, 4], 2)  // [1, 2]
collections.drop([1, 2, 3, 4], 2)  // [3, 4]
```

### flatten

```
collections.flatten<T>(arr: [[T]]) [T]
```

Flattens one level of nesting.

```
collections.flatten([[1, 2], [3], [4, 5]])    // [1, 2, 3, 4, 5]
```

## Combining

### zip

```
collections.zip<A, B>(a: [A], b: [B]) [Pair<A, B>]
```

Pairs elements by index. Result length equals the shorter array.

```
let names = ["alice", "bob"]
let scores = [95, 87]
let pairs = collections.zip(names, scores)
// pairs[0].first == "alice", pairs[0].second == 95
```

### enumerate

```
collections.enumerate<T>(arr: [T]) [Pair<int, T>]
```

Pairs each element with its index.

```
let indexed = collections.enumerate(["a", "b", "c"])
// Pair { first: 0, second: "a" }, Pair { first: 1, second: "b" }, ...
```

## Side Effects

### for_each

```
collections.for_each<T>(arr: [T], f: fn(T))
```

Calls `f` on each element. Returns void.

```
collections.for_each(["alice", "bob"], (name: string) => {
    print("hello {name}")
})
```

## Example: Pipeline

```
import std.collections

fn main() {
    let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    let even_data = collections.filter(data, (x: int) => x % 2 == 0)
    let squared = collections.map(even_data, (x: int) => x * x)
    let total = collections.fold(squared, 0, (acc: int, x: int) => acc + x)
    print("sum of even squares: {total}")
    // sum of even squares: 220
}
```
