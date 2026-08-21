# Expressions

An expression computes a value. Every expression has a type, determined at compile time.

## Evaluation Order

Expressions are evaluated left-to-right. In a function call `foo(a(), b(), c())`, `a()` is evaluated first, then `b()`, then `c()`, then `foo` is called with the results. This order is guaranteed and programs may rely on it.

## Operator Precedence

Operators are listed from lowest to highest precedence. Operators at the same precedence level are left-associative unless noted otherwise.

| Precedence | Operators | Description |
|-----------|-----------|-------------|
| 1 (lowest) | `\|\|` | Logical OR |
| 2 | `&&` | Logical AND |
| 3 | `\|` | Bitwise OR |
| 4 | `^` | Bitwise XOR |
| 5 | `&` | Bitwise AND |
| 6 | `==` `!=` | Equality |
| 7 | `<` `>` `<=` `>=` | Comparison |
| 8 | `<<` `>>` | Bit shift |
| 9 | `..` `..=` | Range |
| 10 | `+` `-` | Addition, subtraction |
| 11 | `*` `/` `%` | Multiplication, division, remainder |
| 12 | `as` | Type cast |
| 13 | `-` `!` `~` | Prefix: negation, logical NOT, bitwise NOT |
| 14 (highest) | `.` `[]` `?` `!` `catch` | Postfix: access, index, propagation |

### Precedence Notes

Prefix operators bind tighter than all binary operators but looser than postfix operators. This means `-x.field` parses as `-(x.field)`, not `(-x).field`.

The `as` operator binds tighter than arithmetic, so `x + y as float` parses as `x + (y as float)`.

Range operators (`..` and `..=`) bind looser than arithmetic, so `1 + 2..10` parses as `(1 + 2)..10`.

## Comparison Chaining

Comparison operators can be chained. The expression `a < b < c` is equivalent to `a < b && b < c`, with `b` evaluated only once.

Any combination of comparison operators can be chained:

```
0 < x <= 100         // 0 < x && x <= 100
a == b == c           // a == b && b == c
x >= 0 < y            // x >= 0 && 0 < y (unusual but valid)
```

Chaining only applies to comparison operators (`<`, `>`, `<=`, `>=`, `==`, `!=`). Other binary operators cannot be chained.

## Arithmetic Operators

### Binary Arithmetic

| Operator | Types | Result |
|----------|-------|--------|
| `+` | `int`, `int` | `int` |
| `+` | `float`, `float` | `float` |
| `+` | `string`, `string` | `string` (concatenation) |
| `-` | `int`, `int` | `int` |
| `-` | `float`, `float` | `float` |
| `*` | `int`, `int` | `int` |
| `*` | `float`, `float` | `float` |
| `/` | `int`, `int` | `int` (truncates toward zero) |
| `/` | `float`, `float` | `float` |
| `%` | `int`, `int` | `int` (sign follows dividend) |
| `%` | `float`, `float` | `float` |

Integer division by zero is a contract violation that aborts the program. Floating-point division by zero produces `Infinity`, `-Infinity`, or `NaN` per IEEE 754.

Integer overflow wraps silently using two's complement.

### Unary Arithmetic

| Operator | Type | Result |
|----------|------|--------|
| `-` (prefix) | `int` | `int` (negation) |
| `-` (prefix) | `float` | `float` (negation) |

## Logical Operators

| Operator | Description | Short-circuit |
|----------|-------------|---------------|
| `&&` | Logical AND | Yes — right operand not evaluated if left is `false` |
| `\|\|` | Logical OR | Yes — right operand not evaluated if left is `true` |
| `!` (prefix) | Logical NOT | N/A |

All logical operators require `bool` operands and produce `bool` results.

## Bitwise Operators

| Operator | Types | Result |
|----------|-------|--------|
| `&` | `int`, `int` | `int` |
| `\|` | `int`, `int` | `int` |
| `^` | `int`, `int` | `int` |
| `~` (prefix) | `int` | `int` |
| `<<` | `int`, `int` | `int` |
| `>>` | `int`, `int` | `int` (arithmetic shift, sign-extending) |

Bitwise operators only accept `int` operands.

Right shift (`>>`) is an arithmetic shift that preserves the sign bit. It is parsed as two consecutive `>` tokens to avoid ambiguity with nested generic type arguments.

## Comparison Operators

| Operator | Types | Result |
|----------|-------|--------|
| `==` `!=` | any two compatible types | `bool` |
| `<` `>` `<=` `>=` | `int`, `int` | `bool` |
| `<` `>` `<=` `>=` | `float`, `float` | `bool` |
| `<` `>` `<=` `>=` | `string`, `string` | `bool` (lexicographic) |

Equality comparison (`==`, `!=`) is defined for all types. For classes, it compares by reference identity. For primitives and strings, it compares by value.

## Type Cast

The `as` operator performs an explicit type conversion:

```
let x = 42 as float     // 42.0
let y = 3.7 as int      // 3
let z = 1 as bool       // true
```

See the [Types](types.md) chapter for the complete list of allowed casts.

## Assignment Operators

### Compound Assignment

| Operator | Equivalent |
|----------|-----------|
| `x += y` | `x = x + y` |
| `x -= y` | `x = x - y` |
| `x *= y` | `x = x * y` |
| `x /= y` | `x = x / y` |
| `x %= y` | `x = x % y` |

Compound assignment operators are statements, not expressions. They do not produce a value.

### Increment and Decrement

| Operator | Equivalent |
|----------|-----------|
| `x++` | `x = x + 1` |
| `x--` | `x = x - 1` |

Increment and decrement are statements, not expressions. They cannot be used in expression position — `y = x++` is a compile error.

## Function Calls

```
function_name(arg1, arg2, arg3)
function_name(arg1, arg2, arg3,)    // trailing comma allowed
function_name()                      // no arguments
```

Arguments are evaluated left-to-right before the function is called.

Trailing commas are permitted in argument lists.

### Generic Function Calls

Functions with type parameters can be called with explicit type arguments:

```
identity<int>(42)
first<string>(names)
```

When type arguments can be inferred from the value arguments, they may be omitted:

```
identity(42)     // T inferred as int
first(names)     // T inferred as string
```

## Method Calls

```
object.method(args)
```

Method calls use dot syntax. The object expression is evaluated first, then the arguments left-to-right, then the method is called.

### Chaining Across Lines

A `.` at the beginning of a line continues the expression from the previous line, enabling multi-line method chains:

```
users
    .filter((u: User) => u.active)
    .map((u: User) => u.name)
    .sort()
```

## Field Access

```
object.field_name
```

Accesses a named field on a class instance. The field must be defined in the class declaration.

## Indexing

```
array[index]
map[key]
string[index]
```

The index expression must have the appropriate type for the container:
- Arrays: `int` index, returns element type
- Maps: key type, returns value type
- Strings: `int` index, returns `string` (single character)

## String Interpolation

Interpolated strings are prefixed with `f` and allow arbitrary expressions inside `{` `}`:

```
f"Hello, {name}!"
f"Sum: {a + b}"
f"Status: {if active { "on" } else { "off" }}"
f"Items: {items.len()}"
```

Expressions inside `{}` are evaluated at runtime and converted to their string representation. Any valid Pluto expression may appear inside the braces.

## Closures

Closures are anonymous functions defined with arrow syntax:

```
(x: int) => x + 1                          // single expression body
(x: int, y: int) => x * y                  // multiple parameters
(x: int) => { let y = x + 1; return y }    // block body
() => print("hello")                        // no parameters
```

If the body is a single expression, it is implicitly returned. If the body is a block (delimited by `{` `}`), it follows normal function body rules and may contain multiple statements.

### Parameter Types

All closure parameters must have explicit type annotations. Type inference for closure parameters is not supported.

```
(x: int) => x + 1       // valid
(x) => x + 1            // compile error: missing type
```

### Return Type

The return type of a closure is inferred from the body. An explicit return type annotation can be provided between the parameters and the arrow:

```
(x: int) int => x + 1
(x: int) string => x.to_string()
```

### Capture

Closures capture variables from their enclosing scope by value. The captured value is a snapshot at the time the closure is created.

```
let x = 10
let f = () => x + 1    // captures x by value (10)
x = 20                  // does not affect f
f()                     // returns 11, not 21
```

For heap-allocated types (strings, arrays, classes), capture by value copies the reference, not the underlying data. Mutations to the underlying data through the original variable will be visible through the closure's reference.

The compiler automatically determines which variables are captured. There is no explicit capture list.

### Recursive Closures

A closure assigned to a variable may call itself through that variable:

```
let factorial: fn(int) int = (n: int) => {
    if n <= 1 {
        return 1
    }
    return n * factorial(n - 1)
}
```

The variable must have an explicit type annotation when used recursively.

## Struct Literals

Struct literals create instances of classes:

```
Point { x: 1.0, y: 2.0 }
User { name: "Alice", age: 30 }
Box<int> { value: 42 }
```

Field order in the literal does not need to match the declaration order. All fields must be provided — there are no default values.

Struct literals are expressions and may appear in any expression position:

```
let p = Point { x: 0.0, y: 0.0 }
return Point { x: 1.0, y: 2.0 }
foo(Point { x: 3.0, y: 4.0 })
```

Trailing commas are permitted in field lists.

## Enum Constructors

### Unit Variants

Unit variants are referenced with dot syntax:

```
Color.Red
Direction.North
```

### Data Variants

Data-carrying variants use struct literal syntax after the variant name:

```
Option.Some { value: 42 }
Shape.Circle { radius: 3.14 }
Result<int, string>.Ok { value: 100 }
```

## Array Literals

```
[1, 2, 3]
["a", "b", "c"]
[]                    // empty array (type inferred from context)
[x, y, z,]           // trailing comma allowed
```

All elements must have the same type. The type of the array is inferred from the elements, or from the surrounding context for empty arrays.

## Map Literals

```
Map<string, int> { "a": 1, "b": 2 }
Map<int, string> {}
```

Map literals require explicit type arguments. Entries are written as `key: value` pairs separated by commas.

## Set Literals

```
Set<int> { 1, 2, 3 }
Set<string> {}
```

Set literals require explicit type arguments. Elements are separated by commas.

## Range Expressions

```
start..end       // exclusive: start, start+1, ..., end-1
start..=end      // inclusive: start, start+1, ..., end
```

Both `start` and `end` must be `int` expressions. Ranges are first-class values of type `Range` and can be stored in variables, passed to functions, or used in `for` loops.

## If Expressions

When used in expression position, `if` produces a value. The `else` branch is required:

```
let x = if condition { 1 } else { 2 }
let msg = if count == 0 { "none" } else { f"{count} items" }
```

Both branches must produce values of compatible types. The result type is the common type of the two branches.

If expressions can appear in any expression position:

```
foo(if x { a } else { b })
return if valid { result } else { none }
```

## Match Expressions

Match expressions destructure a value and produce a result based on which pattern matches:

```
let name = match color {
    Color.Red => "red"
    Color.Green => "green"
    Color.Blue => "blue"
}
```

Data-carrying variants bind their fields:

```
let description = match shape {
    Shape.Circle { radius: r } => f"circle with radius {r}"
    Shape.Rectangle { width: w, height: h } => f"{w}x{h} rectangle"
}
```

Match expressions must be exhaustive — all variants of the enum must be covered.

Match arms use `=>` followed by an expression. All arms must produce values of compatible types.

## Spawn Expressions

The `spawn` keyword creates a concurrent task:

```
let task = spawn compute(42)
let result = task.get()
```

`spawn` takes a function call and returns a `Task<T>` where `T` is the return type of the function. See the [Concurrency](concurrency.md) chapter.

## Error Propagation

The postfix `!` operator propagates errors from a fallible function call:

```
let value = get_data()!    // if get_data() raises, propagate the error
```

The `!` operator can only appear in functions that are themselves fallible. See the [Error Handling](error-handling.md) chapter.

## Error Handling

The `catch` keyword handles errors from a fallible expression:

```
// Shorthand: provide a default value
let value = get_data() catch default_value

// Block: handle with custom logic
let value = get_data() catch e {
    log(f"Error: {e}")
    return fallback
}
```

See the [Error Handling](error-handling.md) chapter.

## Null Propagation

The postfix `?` operator unwraps a nullable value or propagates `none`:

```
let name = find_user(id)?    // if none, return none from enclosing function
```

The `?` operator can only appear in functions whose return type is nullable. See the [Types](types.md) chapter.

## Static Trait Method Calls

Static methods on traits are called with double-colon syntax and explicit type arguments:

```
TypeInfo::type_name<User>()
TypeInfo::kind<int>()
```

## Parenthesized Expressions

Parentheses override precedence:

```
(a + b) * c
```

A parenthesized expression evaluates to the same value and type as the inner expression.
