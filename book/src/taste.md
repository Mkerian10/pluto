# A Taste of Pluto

This chapter walks through a complete Pluto program -- an order-processing service with dependency injection, error handling, and an app entry point. The goal is to show how Pluto's features work together in practice, not to explain every detail. Subsequent chapters cover each construct in depth.

## The program

```
error OrderError {
    reason: string
}

class OrderValidator {
    fn validate(self, amount: int) {
        if amount <= 0 {
            raise OrderError { reason: "amount must be positive" }
        }
    }
}

class OrderService[validator: OrderValidator] {
    processed: int

    fn process(self, id: string, amount: int) string {
        self.validator.validate(amount)!
        return "Order {id} processed for {amount}"
    }
}

app OrderSystem[svc: OrderService] {
    fn main(self) {
        let result = self.svc.process("ORD-1", 100) catch err {
            print("Failed: order rejected")
            return
        }
        print(result)
    }
}
```

Save this as `main.pluto` and run it:

```
$ plutoc run main.pluto
Order ORD-1 processed for 100
```

That is a complete, runnable program. No main function signature to memorize, no framework to initialize, no dependency container to configure. The compiler handles all of it.

## Breaking it down

**Error declaration.** `error OrderError` defines a typed error with a `reason` field. Errors in Pluto are not exceptions, not sum types, and not integer codes. They are their own language concept -- declared at the top level, raised explicitly, and tracked by the compiler through the entire call graph.

```
error OrderError {
    reason: string
}
```

**Classes and methods.** `OrderValidator` is a class with a method. Nothing surprising here -- but notice that `validate` does not declare that it can raise an error. The compiler infers this from the `raise` statement inside the body. You never annotate fallibility.

```
class OrderValidator {
    fn validate(self, amount: int) {
        if amount <= 0 {
            raise OrderError { reason: "amount must be positive" }
        }
    }
}
```

**Bracket dependencies.** `OrderService[validator: OrderValidator]` declares that `OrderService` depends on an `OrderValidator`. The compiler will create the validator, create the service, and inject the dependency -- all at compile time. There is no runtime container, no reflection, no `@Inject` annotation. The `validator` field is available on `self` like any other field.

```
class OrderService[validator: OrderValidator] {
    processed: int

    fn process(self, id: string, amount: int) string {
        self.validator.validate(amount)!
        return "Order {id} processed for {amount}"
    }
}
```

**Error propagation with `!`.** The `!` after `self.validator.validate(amount)` means: if this call raises an error, propagate it to my caller. The compiler knows that `validate` can raise `OrderError` (it inferred this), and it knows that `process` now can too (it inferred this as well). The `!` is the only annotation you write, and it is an explicit decision to propagate rather than handle.

**String interpolation.** `"Order {id} processed for {amount}"` embeds expressions directly in the string. Any expression works inside the braces.

**The app declaration.** `app OrderSystem[svc: OrderService]` is the root of the program. It declares its dependencies in brackets, just like a class. The compiler builds the full dependency graph -- `OrderSystem` needs `OrderService`, which needs `OrderValidator` -- performs a topological sort, allocates everything, wires it together, and calls `main`.

```
app OrderSystem[svc: OrderService] {
    fn main(self) {
        let result = self.svc.process("ORD-1", 100) catch err {
            print("Failed: order rejected")
            return
        }
        print(result)
    }
}
```

**Error handling with `catch`.** The `catch err { ... }` block handles any error raised by `process`. Inside the block, you can execute arbitrary statements -- here we print a message and return from main. If you wrote `catch "default"` instead, the catch expression would evaluate to the string `"default"` as a fallback value. Both forms exist because error handling sometimes means recovering with a value and sometimes means bailing out.

## The same program in Go

For comparison, here is the equivalent Go program. This is not a contrived strawman -- it is what you would actually write.

```go
package main

import "fmt"

type OrderError struct {
    Reason string
}

func (e *OrderError) Error() string {
    return e.Reason
}

type OrderValidator struct{}

func (v *OrderValidator) Validate(amount int) error {
    if amount <= 0 {
        return &OrderError{Reason: "amount must be positive"}
    }
    return nil
}

type OrderService struct {
    Validator *OrderValidator
    Processed int
}

func (s *OrderService) Process(id string, amount int) (string, error) {
    if err := s.Validator.Validate(amount); err != nil {
        return "", err
    }
    return fmt.Sprintf("Order %s processed for %d", id, amount), nil
}

func main() {
    validator := &OrderValidator{}
    svc := &OrderService{
        Validator: validator,
        Processed: 0,
    }

    result, err := svc.Process("ORD-1", 100)
    if err != nil {
        fmt.Printf("Failed: order rejected\n")
        return
    }
    fmt.Println(result)
}
```

The Go version is roughly twice as long, and the additional lines are not doing interesting work. They are: implementing the `error` interface, returning tuples, writing `if err != nil`, and manually constructing and wiring dependencies in `main`. Every Go backend team writes this scaffolding, and every team writes it slightly differently.

In Pluto, these decisions are made once -- in the language -- and enforced by the compiler. The error interface is replaced by a typed error declaration. The tuple return is replaced by compiler-tracked fallibility. The `if err != nil` is replaced by `!` and `catch`. The manual wiring in `main` is replaced by bracket dependencies and the `app` construct.

## What comes next

The chapters in Part 2 cover each of Pluto's distinguishing features in detail: error handling, dependency injection, the app model, concurrency, and contracts. If you need a quick refresher on variables, functions, or control flow, see Chapter 8: Syntax at a Glance.
