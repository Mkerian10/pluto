# std.math

The `std.math` module provides basic math utility functions.

```
import std.math
```

## Functions

### abs

```
math.abs(x: int) int
```

Returns the absolute value:

```
print(math.abs(-5))     // 5
print(math.abs(3))      // 3
```

### min / max

```
math.min(a: int, b: int) int
math.max(a: int, b: int) int
```

```
print(math.min(3, 7))   // 3
print(math.max(3, 7))   // 7
```

### pow

```
math.pow(base: int, exp: int) int
```

Integer exponentiation:

```
print(math.pow(2, 10))  // 1024
print(math.pow(3, 3))   // 27
```

### clamp

```
math.clamp(x: int, lo: int, hi: int) int
```

Clamps `x` to the range `[lo, hi]`:

```
print(math.clamp(5, 0, 10))     // 5
print(math.clamp(-3, 0, 10))    // 0
print(math.clamp(15, 0, 10))    // 10
```
