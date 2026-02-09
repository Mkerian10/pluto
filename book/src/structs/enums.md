# Enums and Pattern Matching

Enums let you define a type that can be one of several variants.

## Unit Variants

The simplest enums have variants with no data:

```
enum Color {
    Red
    Green
    Blue
}

fn main() {
    let c = Color.Red
    print(c)
}
```

## Data-Carrying Variants

Variants can carry data:

```
enum Shape {
    Circle { radius: int }
    Rectangle { width: int, height: int }
}

fn main() {
    let s = Shape.Circle { radius: 5 }
}
```

You can mix unit and data variants in the same enum:

```
enum Token {
    Number { val: int }
    Plus
    Eof
}
```

## Pattern Matching with match

Use `match` to handle each variant of an enum:

```
enum Color {
    Red
    Green
    Blue
}

fn describe(c: Color) {
    match c {
        Color.Red {
            print("the color is red")
        }
        Color.Green {
            print("the color is green")
        }
        Color.Blue {
            print("the color is blue")
        }
    }
}
```

## Destructuring Data Variants

When matching data variants, you can bind the fields to variables:

```
enum Shape {
    Circle { radius: int }
    Square { side: int }
}

fn area(s: Shape) int {
    match s {
        Shape.Circle { radius } {
            return 3 * radius * radius
        }
        Shape.Square { side } {
            return side * side
        }
    }
}

fn main() {
    let c = Shape.Circle { radius: 5 }
    print(area(c))      // 75

    let s = Shape.Square { side: 4 }
    print(area(s))      // 16
}
```

## Exhaustiveness Checking

The compiler ensures that `match` covers every variant. If you forget one, you'll get a compile error:

```
enum Direction {
    North
    South
    East
    West
}

fn describe(d: Direction) {
    match d {
        Direction.North { print("north") }
        Direction.South { print("south") }
        // Error: non-exhaustive match -- East and West not covered
    }
}
```

## Enums as Function Parameters and Return Types

```
enum Status {
    Active
    Suspended { reason: string }
}

fn check_status() Status {
    return Status.Suspended { reason: "maintenance" }
}

fn main() {
    let s = check_status()
    match s {
        Status.Active {
            print("all good")
        }
        Status.Suspended { reason } {
            print("suspended: {reason}")
        }
    }
}
```

## Enums with Methods

Enums don't have methods directly, but you can write functions that operate on them:

```
enum Direction {
    North
    South
    East
    West
}

fn opposite(d: Direction) Direction {
    match d {
        Direction.North { return Direction.South }
        Direction.South { return Direction.North }
        Direction.East { return Direction.West }
        Direction.West { return Direction.East }
    }
}
```
