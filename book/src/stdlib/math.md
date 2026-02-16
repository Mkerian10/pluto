# std.math

Integer and float math utilities. Built-in float operations (`sqrt`, `sin`, `cos`, `floor`, etc.) are available without importing this module.

```
import std.math
```

## Integer Functions

### abs

```
math.abs(x: int) int
```

```
math.abs(-5)    // 5
math.abs(3)     // 3
```

### min / max

```
math.min(a: int, b: int) int
math.max(a: int, b: int) int
```

```
math.min(3, 7)    // 3
math.max(3, 7)    // 7
```

### clamp

```
math.clamp(x: int, lo: int, hi: int) int
```

Constrains `x` to the range `[lo, hi]`.

```
math.clamp(5, 0, 10)      // 5
math.clamp(-3, 0, 10)     // 0
math.clamp(15, 0, 10)     // 10
```

### pow

```
math.pow(base: int, exp: int) int
```

Integer exponentiation. Returns 0 for negative exponents.

```
math.pow(2, 10)    // 1024
math.pow(3, 3)     // 27
```

### sign

```
math.sign(x: int) int
```

Returns 1, 0, or -1.

```
math.sign(42)     // 1
math.sign(0)      // 0
math.sign(-7)     // -1
```

### gcd / lcm

```
math.gcd(a: int, b: int) int
math.lcm(a: int, b: int) int
```

```
math.gcd(12, 8)    // 4
math.lcm(4, 6)     // 12
```

### factorial

```
math.factorial(n: int) int
```

```
math.factorial(5)    // 120
```

### is_even / is_odd

```
math.is_even(n: int) bool
math.is_odd(n: int) bool
```

```
math.is_even(4)    // true
math.is_odd(3)     // true
```

## Float Functions

### clamp_float

```
math.clamp_float(x: float, lo: float, hi: float) float
```

```
math.clamp_float(1.5, 0.0, 1.0)    // 1.0
```

### to_radians / to_degrees

```
math.to_radians(degrees: float) float
math.to_degrees(radians: float) float
```

```
math.to_radians(180.0)    // 3.14159...
math.to_degrees(3.14159265358979323846)    // 180.0
```

## Constants

```
math.PI() float      // 3.14159265358979323846
math.E() float       // 2.71828182845904523536
math.TAU() float     // 6.28318530717958647692
```

Note: These are functions that return the constant values.

## Example

```
import std.math

fn main() {
    let n = -42
    print("{math.abs(n)}")             // 42
    print("{math.clamp(n, 0, 100)}")   // 0
    print("{math.gcd(48, 18)}")        // 6
    print("{math.factorial(6)}")       // 720
}
```
