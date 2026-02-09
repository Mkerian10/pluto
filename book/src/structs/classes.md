# Classes

## Defining a Class

Classes are defined with `class`, followed by fields in curly braces:

```
class Point {
    x: int
    y: int
}
```

## Creating Instances

Use struct-literal syntax to create an instance:

```
fn main() {
    let p = Point { x: 10, y: 20 }
    print(p.x)     // 10
    print(p.y)     // 20
}
```

## Methods

Methods are defined inside the class body and take `self` as their first parameter:

```
class Point {
    x: int
    y: int

    fn sum(self) int {
        return self.x + self.y
    }

    fn distance_to(self, other: Point) int {
        let dx = self.x - other.x
        let dy = self.y - other.y
        return dx * dx + dy * dy
    }
}

fn main() {
    let p = Point { x: 3, y: 4 }
    print(p.sum())              // 7

    let q = Point { x: 0, y: 0 }
    print(p.distance_to(q))    // 25
}
```

## Field Mutation

You can mutate fields directly on a class instance:

```
class Counter {
    val: int

    fn get(self) int {
        return self.val
    }
}

fn main() {
    let c = Counter { val: 0 }
    c.val = 42
    print(c.get())      // 42
}
```

## Classes as Parameters

Classes work as function parameters and return types:

```
class Rect {
    width: int
    height: int

    fn area(self) int {
        return self.width * self.height
    }
}

fn bigger(a: Rect, b: Rect) Rect {
    if a.area() > b.area() {
        return a
    }
    return b
}

fn main() {
    let a = Rect { width: 3, height: 4 }
    let b = Rect { width: 2, height: 7 }
    let big = bigger(a, b)
    print(big.area())   // 14
}
```

## Classes with Class Fields

Classes can contain other classes as fields:

```
class Address {
    city: string
}

class Person {
    name: string
    address: Address
}

fn main() {
    let p = Person {
        name: "alice",
        address: Address { city: "new york" }
    }
    print(p.address.city)   // "new york"
}
```

## Method Chaining

Methods that return `self` or the same class type can be chained:

```
class Builder {
    val: int

    fn add(self, n: int) Builder {
        return Builder { val: self.val + n }
    }
}

fn main() {
    let b = Builder { val: 0 }
    let result = b.add(1).add(2).add(3)
    print(result.val)    // 6
}
```
