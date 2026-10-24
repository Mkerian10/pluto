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
    print(f"hello {name}")
})
```

## Sorting

All sorts return a new array and leave the input untouched. They are stable (equal elements keep their order) and run in O(n log n).

### sort_by

```
collections.sort_by<T>(arr: [T], less: fn(T, T) bool) [T]
```

Sorts with `less(a, b)`, which returns true when `a` should come strictly before `b`.

```
let desc = collections.sort_by([1, 3, 2], (a: int, b: int) => a > b)    // [3, 2, 1]
```

### sort / sort_floats / sort_strings

```
collections.sort(arr: [int]) [int]
collections.sort_floats(arr: [float]) [float]
collections.sort_strings(arr: [string]) [string]
```

Ascending order. Strings sort byte-wise, as `strings.compare` does.

```
collections.sort([3, 1, 2])                          // [1, 2, 3]
collections.sort_strings(["pear", "apple", "fig"])   // ["apple", "fig", "pear"]
```

## Fallible Callbacks

```
collections.try_map<T, U>(arr: [T], f: fn(T) U!) [U]
collections.try_filter<T>(arr: [T], f: fn(T) bool!) [T]
collections.try_fold<T, U>(arr: [T], initial: U, f: fn(U, T) U!) U
collections.try_for_each<T>(arr: [T], f: fn(T)!)
```

Variants of `map`, `filter`, `fold` and `for_each` whose callback may raise. The first error stops the walk and propagates to the caller, so a call must be handled with `!` or `catch`. Infallible callbacks are accepted too.

```
let halves = collections.try_map([4, 8], halve)!    // [2, 4]
let safe = collections.try_map(inputs, halve) catch err {
    let none_left: [int] = []
    none_left
}
```

## Example: Pipeline

```
import std.collections

fn main() {
    let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    let even_data = collections.filter(data, (x: int) => x % 2 == 0)
    let squared = collections.map(even_data, (x: int) => x * x)
    let total = collections.fold(squared, 0, (acc: int, x: int) => acc + x)
    print(f"sum of even squares: {total}")
    // sum of even squares: 220
}
```
