# Traits

Traits define shared behavior that classes can implement. They're similar to interfaces in other languages, but with support for default methods and polymorphic dispatch.

## Defining a Trait

```
trait HasArea {
    fn area(self) int
}
```

A trait declares method signatures that implementing classes must provide.

## Implementing a Trait

Use `impl` after the class name:

```
class Circle impl HasArea {
    radius: int

    fn area(self) int {
        return 3 * self.radius * self.radius
    }
}

class Square impl HasArea {
    side: int

    fn area(self) int {
        return self.side * self.side
    }
}
```

## Trait-Based Polymorphism

The real power of traits is using them as function parameter types. This lets you write code that works with any class that implements the trait:

```
fn print_area(shape: HasArea) {
    print(shape.area())
}

fn main() {
    let c = Circle { radius: 5 }
    let s = Square { side: 4 }

    print_area(c)   // 75
    print_area(s)   // 16
}
```

Under the hood, Pluto uses vtable-based dynamic dispatch for trait parameters.

## Implementing Multiple Traits

A class can implement multiple traits by listing them after `impl`:

```
trait HasX {
    fn get_x(self) int
}

trait HasY {
    fn get_y(self) int
}

class Point impl HasX, HasY {
    x: int
    y: int

    fn get_x(self) int {
        return self.x
    }

    fn get_y(self) int {
        return self.y
    }
}
```

## Default Methods

Traits can provide default implementations for methods:

```
trait Greetable {
    fn greet(self) string {
        return "hello"
    }
}

class User impl Greetable {
    name: string
    // Uses the default greet() implementation
}

class Admin impl Greetable {
    name: string

    fn greet(self) string {
        return "hello, admin {self.name}"
    }
}
```

## Trait-Typed Variables

You can declare variables with a trait type:

```
fn main() {
    let shape: HasArea = Circle { radius: 5 }
    print(shape.area())    // 75
}
```

This is useful when you want to store different concrete types in the same variable.
