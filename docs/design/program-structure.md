# Program Structure

## The App

The fundamental unit of a Pluto program is the `app`. An app declares its dependencies, defines its behavior, and is the unit of compilation and deployment.

```
app OrderService[db: APIDatabase, queue: MessageQueue] {
    fn main(self) {
        // process orders from the queue
        for order in self.queue.subscribe("orders") {
            self.db.insert(order)!
        }
    }
}
```

An app:
- Declares its dependencies via bracket deps `[dep: Type]`
- Has a `main(self)` entry point
- Accesses dependencies through `self`
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

Modules organize code into separate files and namespaces.

```
import math
import utils as u

fn main() {
    let v = math.add(1, 2)
    u.log("result: {v}")
}
```

Key properties:
- `import <name>` loads a sibling file (`<name>.pluto`) or directory (`<name>/`)
- Items must be marked `pub` to be visible across modules (private by default)
- Imported items are accessed via qualified names: `math.add()`, `math.Point { x: 1, y: 2 }`
- Files in the same directory are auto-merged (no import needed)
- Hierarchical imports supported: `import net.http`
- Import aliases: `import utils as u`
- Modules cannot contain `app` declarations (only the entry file can)

### Open Questions

- [ ] How do apps compose across modules?
- [ ] Transitive imports (currently restricted — imported modules can't import other modules)
