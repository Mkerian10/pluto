# std.random

Pseudorandom number generation.

```
import std.random
```

## Functions

### next

```
random.next() int
```

Returns a random integer (full range).

### between

```
random.between(low: int, high: int) int
```

Returns a random integer in `[low, high]` (inclusive).

```
let die = random.between(1, 6)
```

### decimal

```
random.decimal() float
```

Returns a random float in `[0.0, 1.0)`.

### decimal_between

```
random.decimal_between(low: float, high: float) float
```

Returns a random float in `[low, high)`.

```
let temp = random.decimal_between(36.0, 38.0)
```

### coin

```
random.coin() bool
```

Returns `true` or `false` with equal probability.

### seed

```
random.seed(s: int)
```

Seeds the random number generator. Same seed produces the same sequence, useful for reproducible tests.

```
random.seed(42)
let a = random.next()
random.seed(42)
let b = random.next()
// a == b
```

## Example

```
import std.random

fn main() {
    // Roll 5 dice
    let i = 0
    while i < 5 {
        print("Roll: {random.between(1, 6)}")
        i = i + 1
    }

    // Reproducible sequence
    random.seed(99)
    let x = random.between(1, 100)
    let y = random.between(1, 100)
    print("Seeded: {x}, {y}")
}
```
