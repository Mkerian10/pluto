# The App Declaration

The `app` declaration is Pluto's top-level construct for building applications with dependency injection.

## Basic App

```
app Main {
    fn main(self) {
        print("hello from the app")
    }
}
```

When you use an `app` declaration, you don't need a standalone `fn main()`. The app's `main(self)` method is the entry point.

## App with Dependencies

Use bracket deps to inject services into your app:

```
class Database {
    fn query(self, q: string) string {
        return "result: {q}"
    }
}

class UserService[db: Database] {
    fn get_user(self, id: string) string {
        return self.db.query(id)
    }
}

app MyApp[users: UserService] {
    fn main(self) {
        let result = self.users.get_user("42")
        print(result)
    }
}
```

The compiler generates startup code that:
1. Allocates a `Database` singleton
2. Allocates a `UserService` singleton, wiring in the `Database`
3. Allocates the `MyApp` instance, wiring in the `UserService`
4. Calls `MyApp.main()`

## Multiple Dependencies

Apps can depend on multiple services:

```
class Logger {
    fn log(self, msg: string) {
        print(msg)
    }
}

class Database {
    fn query(self) string {
        return "data"
    }
}

class AuthService[db: Database] {
    fn check(self) bool {
        self.db.query()
        return true
    }
}

class ApiService[db: Database, auth: AuthService] {
    fn handle(self) string {
        if self.auth.check() {
            return self.db.query()
        }
        return "unauthorized"
    }
}

app MyApp[api: ApiService, logger: Logger] {
    fn main(self) {
        self.logger.log("starting")
        let result = self.api.handle()
        self.logger.log(result)
    }
}
```

## App with Ambient Dependencies

For cross-cutting concerns, use `ambient` in the app:

```
class Logger {
    fn info(self, msg: string) {
        print(msg)
    }
}

class OrderService uses Logger {
    fn process(self) {
        logger.info("processing order")
    }
}

app MyApp[orders: OrderService] {
    ambient Logger

    fn main(self) {
        logger.info("app starting")
        self.orders.process()
    }
}
```

The `ambient` keyword makes a type available to all classes that declare `uses` for it, without needing to thread it through bracket deps.

## Rules

- There can be at most one `app` declaration per program
- The app must have a `fn main(self)` method
- All dependencies are singletons -- each type is instantiated exactly once
- Circular dependencies are rejected at compile time
- Classes with bracket deps cannot be constructed manually with struct literals
