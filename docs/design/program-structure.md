# Program Structure

## The App

The fundamental unit of a Pluto program is the `app`. An app declares its dependencies, defines its behavior, and is the unit of compilation and deployment.

```
app OrderService {
    inject db: APIDatabase
    inject queue: MessageQueue

    fn main() {
        let ch = chan<Order>()
        spawn process_orders(ch, self.db)

        for order in self.queue.subscribe("orders") {
            ch <- order
        }
    }
}
```

An app:
- Declares its dependencies via `inject`
- Has a `main()` entry point
- Can spawn processes and create channels
- Does not know or care about infrastructure, scaling, or placement

## Why `app`?

Different languages have different "0th class objects":
- C: the executable
- Java: the JAR / JVM application
- Go: the binary

Pluto's 0th class object is the **app** because:
- It naturally maps to a deployable unit in a distributed system
- It provides a clear boundary for dependency injection
- It gives the compiler a root node for whole-program analysis
- It separates application logic from infrastructure concerns (which live in the orchestration layer)

## Modules

> **Status:** Open design question.
>
> Topics to resolve:
> - Import syntax
> - Visibility modifiers (public/private)
> - Namespacing strategy
> - How modules relate to apps
> - Can a module contain multiple apps?
