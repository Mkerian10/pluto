# Dependency Injection

Pluto has language-level dependency injection. Instead of wiring services together manually or using a runtime DI container, the compiler resolves dependencies at compile time.

## Bracket Dependencies

Classes declare their dependencies using bracket syntax:

```
class Database {
    fn query(self, q: string) string {
        return "result for: {q}"
    }
}

class UserService[db: Database] {
    fn get_user(self, id: int) string {
        return self.db.query("user_{id}")
    }
}
```

The `[db: Database]` syntax means "this class needs a `Database` instance, injected as `self.db`."

## How Injection Works

The compiler does the following at compile time:

1. Collects all classes with bracket deps
2. Builds a dependency graph
3. Topologically sorts the graph (detecting cycles)
4. Generates code to allocate each dependency as a singleton, in the right order
5. Wires them together before `main()` runs

There is no runtime container, no reflection, no configuration files. It's all resolved at compile time.

## Dependency Chains

Dependencies can be arbitrarily deep:

```
class Config {
    fn get(self, key: string) string {
        return "value"
    }
}

class Database[config: Config] {
    fn query(self) string {
        return self.config.get("db_url")
    }
}

class UserService[db: Database] {
    fn find(self) string {
        return self.db.query()
    }
}
```

The compiler resolves the full chain: `Config` -> `Database` -> `UserService`.

## Shared Singletons

If two classes depend on the same type, they share the same instance:

```
class Database {
    fn name(self) string {
        return "shared_db"
    }
}

class ServiceA[db: Database] {
    fn info(self) string {
        return self.db.name()
    }
}

class ServiceB[db: Database] {
    fn info(self) string {
        return self.db.name()
    }
}
```

Both `ServiceA` and `ServiceB` receive the exact same `Database` instance.

## Regular Fields + Injected Fields

Classes can have both regular fields and injected dependencies:

```
class Logger {
    fn log(self, msg: string) {
        print(msg)
    }
}

class Service[logger: Logger] {
    count: int

    fn run(self) {
        self.logger.log("running")
        print(self.count)
    }
}
```

The key difference: injected fields (in brackets) are wired automatically by the compiler. Regular fields are set when you construct the class manually. Classes with bracket deps cannot be constructed with struct literals -- they can only be created by the DI system.

## Cycle Detection

The compiler rejects circular dependencies at compile time:

```
class A[b: B] {}
class B[a: A] {}
// Error: circular dependency detected
```

## Ambient Dependencies

For cross-cutting concerns like logging that many classes need, Pluto supports ambient injection with `uses`:

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
```

With `uses`, the class accesses the dependency directly by name (`logger`) rather than through `self`. The `ambient` keyword in the app declaration makes the type available:

```
app MyApp[svc: OrderService] {
    ambient Logger

    fn main(self) {
        self.svc.process()
    }
}
```
