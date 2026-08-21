# Statements

Statements are executed for their effects. Unlike expressions, statements do not produce values.

Statements are terminated by newlines. A newline after a complete statement ends it. Newlines inside delimiters (`()`, `{}`, `[]`) and after certain continuation tokens (`.`, binary operators) are ignored.

## Variable Declaration

```
let name = expression
let name: Type = expression
let mut name = expression
let mut name: Type = expression
```

A `let` statement declares a new variable and binds it to a value. An initializer expression is always required — uninitialized variables are not permitted.

Variables are immutable by default. The `mut` keyword makes a variable mutable, allowing subsequent assignment.

```
let x = 42              // immutable, type inferred as int
let y: float = 3.14     // immutable, explicit type
let mut count = 0        // mutable
count = count + 1        // ok — count is mutable
x = 10                   // compile error — x is immutable
```

The type annotation is always optional. The compiler infers the type from the initializer expression. When present, the annotation is checked against the inferred type.

## Assignment

```
variable = expression
```

Assignment updates a mutable variable. Assigning to an immutable variable is a compile error.

```
let mut x = 0
x = 42        // ok
```

### Field Assignment

```
object.field = expression
```

Assigns a value to a field on a class instance. The enclosing method must take `mut self`.

```
fn set_name(mut self, name: string) {
    self.name = name
}
```

### Index Assignment

```
container[index] = expression
```

Assigns a value to an element in an array, map, or other indexable container.

```
let mut arr = [1, 2, 3]
arr[0] = 10

let mut m = Map<string, int> { "a": 1 }
m["b"] = 2
```

### Compound Assignment

```
target += expression
target -= expression
target *= expression
target /= expression
target %= expression
```

Compound assignment operators combine a binary operation with assignment. `x += y` is equivalent to `x = x + y`.

Compound assignment works on variables, fields, and index expressions:

```
count += 1
self.total *= 2
arr[i] += delta
map["key"] -= 1
```

Compound assignment operators are statements, not expressions.

### Increment and Decrement

```
variable++
variable--
```

`x++` is equivalent to `x = x + 1`. `x--` is equivalent to `x = x - 1`.

Increment and decrement are statements, not expressions. They cannot appear in expression position — `y = x++` is a compile error.

## If Statement

```
if condition {
    body
}

if condition {
    body
} else {
    body
}

if condition {
    body
} else if condition {
    body
} else {
    body
}
```

The condition must be an expression of type `bool`. Parentheses around the condition are not required (and by convention, not used). Braces are always required.

The `else if` form chains multiple conditions. There is no `elif` or `elsif` keyword — it is simply `else` followed by another `if`.

When `if` is used as an expression (see [Expressions](expressions.md)), the `else` branch is required and both branches must produce values of compatible types.

## While Loop

```
while condition {
    body
}
```

Executes the body repeatedly as long as the condition evaluates to `true`. The condition is checked before each iteration.

There is no `do-while` loop. Use `while true { ... if !condition { break } }` for loops that must execute at least once.

## For Loop

```
for variable in iterable {
    body
}
```

Iterates over each element in the iterable expression, binding it to the loop variable. The loop variable is immutable and scoped to the loop body.

Iterable expressions include:
- Arrays: iterates over elements
- Ranges: iterates over integer values
- Strings: iterates over characters (as single-character strings)
- Receivers: iterates over channel values until closed

```
for i in 0..10 {
    print(i)           // 0, 1, 2, ..., 9
}

for name in names {
    print(name)
}

for ch in "hello" {
    print(ch)          // "h", "e", "l", "l", "o"
}
```

The loop variable binds a single value per iteration. There is no destructuring in `for` loops — use field access on the bound variable:

```
for pair in enumerate(items) {
    print(f"{pair.first}: {pair.second}")
}
```

## Break and Continue

```
break
continue
```

`break` exits the innermost enclosing loop immediately. `continue` skips to the next iteration of the innermost enclosing loop.

Using `break` or `continue` outside a loop is a compile error.

There are no labeled loops — `break` and `continue` always affect the innermost loop. For nested loop control, extract the inner loop into a function or use a flag variable.

## Return

```
return
return expression
```

`return` exits the enclosing function. If the function has a non-void return type, an expression must be provided. If the function returns void, the expression must be omitted.

The last expression in a function body is NOT implicitly returned — an explicit `return` statement is always required for non-void functions.

```
fn add(a: int, b: int) int {
    return a + b
}

fn greet(name: string) {
    print(f"Hello, {name}!")
    // implicit return (void)
}
```

## Match Statement

```
match expression {
    EnumName.Variant { field: binding } {
        body
    }
    EnumName.Variant {
        body
    }
}
```

Match statements destructure an enum value and execute the body of the matching arm. Match is exclusively for enum types — it does not work on integers, strings, or other types.

### Exhaustiveness

Match statements must be exhaustive — every variant of the enum must have a corresponding arm. The compiler rejects match statements that do not cover all variants.

```
enum Color { Red Green Blue }

match color {
    Color.Red { print("red") }
    Color.Green { print("green") }
    // compile error: missing Color.Blue
}
```

### Binding Fields

Data-carrying variants bind their fields to local variables within the arm body:

```
match shape {
    Shape.Circle { radius: r } {
        print(f"radius: {r}")
    }
    Shape.Rectangle { width: w, height: h } {
        print(f"{w}x{h}")
    }
}
```

Unit variants use an empty binding:

```
match color {
    Color.Red { print("red") }
    Color.Green { print("green") }
    Color.Blue { print("blue") }
}
```

### Match Expressions

Match can also be used as an expression (see [Expressions](expressions.md)). In expression form, arms use `=>` followed by an expression instead of a block:

```
let name = match color {
    Color.Red => "red"
    Color.Green => "green"
    Color.Blue => "blue"
}
```

## Raise Statement

```
raise ErrorName { field: value, ... }
```

Raises an error, immediately exiting the current function and propagating the error to the caller. See the [Error Handling](error-handling.md) chapter.

```
raise NotFoundError { id: "user_123" }
raise ValidationError { field: "email", message: "invalid format" }
```

## Assert Statement

```
assert expression
```

Evaluates the expression. If it is `false`, the program aborts with a diagnostic message. Assert violations are not catchable errors — they indicate programmer bugs.

```
assert items.len() > 0
assert index >= 0
```

## Yield Statement

```
yield expression
```

Produces a value from a generator function, suspending execution until the next value is requested. See the [Concurrency](concurrency.md) chapter.

## Expression Statement

Any expression can be used as a statement by placing it on its own line:

```
print("hello")
foo(x, y)
obj.method()
channel.send(value)
```

The value produced by the expression is discarded.

## Channel Declaration

```
let (sender, receiver) = chan<Type>()
let (sender, receiver) = chan<Type>(capacity)
```

Creates a channel pair for inter-task communication. The sender and receiver are bound to separate variables. See the [Concurrency](concurrency.md) chapter.

```
let (tx, rx) = chan<int>()
let (tx, rx) = chan<string>(100)    // buffered channel
```

## Select Statement

```
select {
    binding = channel.recv() {
        body
    }
    channel.send(value) {
        body
    }
    default {
        body
    }
}
```

Waits for one of several channel operations to complete. See the [Concurrency](concurrency.md) chapter.

## Scope Statement

```
scope(SeedType { fields }) |binding: Type| {
    body
}
```

Creates a scoped dependency injection context. See the [Dependency Injection](dependency-injection.md) chapter.

## Statement Termination

Statements are terminated by newlines. The following rules determine when a newline ends a statement:

1. A newline after a closing brace `}` always ends the statement.
2. A newline after a complete expression ends the statement, unless the next line begins with `.` (method chain continuation).
3. A newline after a binary operator does not end the statement — the expression continues on the next line.
4. Newlines inside matching delimiters (`()`, `[]`, `{}`) are ignored.

These rules allow multi-line expressions without explicit line continuation characters:

```
let result = very_long_expression +
    another_expression

let filtered = users
    .filter((u: User) => u.active)
    .map((u: User) => u.name)

let list = [
    1,
    2,
    3,
]
```
