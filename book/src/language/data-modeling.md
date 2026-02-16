# Data Modeling

Pluto has three data modeling primitives: classes, traits, and enums. There is no inheritance. Composition happens through dependency injection and embedding. Polymorphism happens through traits. This is intentional.

## Classes

A class is a named collection of fields with methods.

```
class Point {
    x: float
    y: float

    fn distance(self, other: Point) float {
        let dx = self.x - other.x
        let dy = self.y - other.y
        return dx * dx + dy * dy
    }

    fn translate(self, dx: float, dy: float) Point {
        return Point { x: self.x + dx, y: self.y + dy }
    }
}
```

Instantiate with struct literal syntax. Methods require `self` as the first parameter:

```
let p = Point { x: 1.0, y: 2.0 }
let q = p.translate(3.0, 4.0)
print(p.distance(q))
```

### Mutation

Use `mut self` to declare a method that mutates fields:

```
class Counter {
    val: int

    fn increment(mut self) {
        self.val = self.val + 1
    }
}
```

The compiler enforces mutability: only `mut self` methods can assign to `self.field`, and callers must hold a mutable reference to call `mut self` methods on an object.

### Composition and Chaining

Classes contain other classes as fields -- this is how you compose, not through inheritance:

```
class Address { city: string }
class Person { name: string, address: Address }

fn main() {
    let p = Person { name: "alice", address: Address { city: "new york" } }
    print(p.address.city)
}
```

Methods returning the same class type can be chained:

```
class Builder {
    val: int
    fn add(self, n: int) Builder { return Builder { val: self.val + n } }
}

fn main() {
    let result = Builder { val: 0 }.add(1).add(2).add(3)
    print(result.val)    // 6
}
```

## Traits

Traits define shared behavior. They are Pluto's mechanism for polymorphism.

### Definition and Implementation

```
trait HasArea {
    fn area(self) int
}

class Circle impl HasArea {
    radius: int
    fn area(self) int { return 3 * self.radius * self.radius }
}

class Square impl HasArea {
    side: int
    fn area(self) int { return self.side * self.side }
}
```

`impl` goes on the class declaration. A class can implement multiple traits:

```
class Point impl HasX, HasY {
    x: int
    y: int
    fn get_x(self) int { return self.x }
    fn get_y(self) int { return self.y }
}
```

The compiler verifies that all required methods are present with correct signatures.

### Structural Typing

Traits use structural typing. If a class has the right methods, it satisfies the trait -- even without an explicit `impl`. The `impl` keyword is documentation and compile-time verification, not a requirement for dispatch.

### Default Methods

Traits can provide default implementations that classes inherit or override:

```
trait Greetable {
    fn greet(self) string { return "hello" }
}

class User impl Greetable { name: string }

class Admin impl Greetable {
    name: string
    fn greet(self) string { return "hello, admin {self.name}" }
}
```

### Trait-Typed Parameters

Use a trait as a parameter type for polymorphic dispatch:

```
fn print_area(shape: HasArea) {
    print(shape.area())
}

fn main() {
    print_area(Circle { radius: 5 })
    print_area(Square { side: 4 })
}
```

This uses vtable-based dynamic dispatch. You can also declare variables with trait types: `let shape: HasArea = Circle { radius: 5 }`.

### Traits with Contracts

Trait methods can carry `requires` and `ensures` clauses, enforced at runtime on all implementors:

```
trait Validator {
    fn validate(self, x: int) int
        requires x > 0
        ensures result > 0
}

class Doubler impl Validator {
    id: int
    fn validate(self, x: int) int { return x * 2 }
}
```

Calling `validate(-1)` aborts with a requires violation. Implementors cannot weaken the contract (Liskov substitution principle).

## Enums

Enums represent a type that is one of several variants.

### Unit and Data-Carrying Variants

```
enum Color { Red, Green, Blue }

enum Shape {
    Circle { radius: float }
    Rect { w: float, h: float }
}

let c = Color.Red
let s = Shape.Circle { radius: 3.14 }
```

You can mix unit and data variants in the same enum.

### Pattern Matching

Use `match` to destructure enums. Data variant bindings go in the first `{ }`, the arm body in the second:

```
fn describe(s: Shape) {
    match s {
        Shape.Circle { radius } {
            print("circle r={radius}")
        }
        Shape.Rect { w, h } {
            print("rect {w}x{h}")
        }
    }
}
```

Unit variants have no bindings -- just the body block:

```
match c {
    Color.Red { print("red") }
    Color.Green { print("green") }
    Color.Blue { print("blue") }
}
```

### Exhaustiveness Checking

The compiler rejects non-exhaustive matches. Miss a variant, get a compile error.

### Enums as Parameters and Return Types

```
enum Result {
    Ok { value: int }
    Err { code: int }
}

fn compute(x: int) Result {
    if x > 0 {
        return Result.Ok { value: x * 2 }
    }
    return Result.Err { code: -1 }
}

fn main() {
    let r = compute(5)
    match r {
        Result.Ok { value } { print(value) }
        Result.Err { code } { print(code) }
    }
}
```

## The Design Philosophy

No inheritance. This is the most important design decision in Pluto's type system. Behavior reuse happens through dependency injection (`class UserService[db: Database]`). Polymorphism happens through traits. Data composition happens through embedding.

There is no `extends`, no `super`, no method resolution order, no diamond problem, no fragile base class problem. The language does not have these concepts, and the programs are simpler for it.
