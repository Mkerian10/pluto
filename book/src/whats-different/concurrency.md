# Concurrency

Go popularized the idea that concurrency should be cheap and communication should use channels. Rust proved that compile-time analysis can eliminate data races. Java continues to evolve threading models with virtual threads and structured concurrency APIs. Each made a different tradeoff between safety, ergonomics, and performance.

Pluto takes a different position: **you write normal classes, and the compiler figures out what needs synchronization.** Tasks give you lightweight concurrency. Channels give you communication. And for shared state -- DI singletons accessed from concurrent contexts -- the compiler infers reader/writer locks from `self` vs `mut self` method signatures. No `sync.Mutex`, no `Arc<RwLock<T>>`, no `synchronized` blocks.

## Tasks

### Spawning and Joining

`spawn` takes a function call and runs it concurrently, returning a `Task<T>` handle:

```
fn double(x: int) int {
    return x * 2
}

fn main() {
    let t1 = spawn double(5)
    let t2 = spawn double(10)
    let t3 = spawn double(15)
    print(t1.get())
    print(t2.get())
    print(t3.get())
}
```

`spawn` returns immediately. The function executes on a separate thread. `.get()` blocks until the result is available. Void functions produce a `Task<void>` -- call `.get()` to wait for completion even when there is no return value.

### Error Handling

Errors flow through tasks transparently. If the spawned function can raise errors, `.get()` propagates them:

```
error NetworkError {
    message: string
}

fn fetch(url: string) string {
    if url == "" {
        raise NetworkError { message: "empty url" }
    }
    return "response from {url}"
}

fn main() {
    let t = spawn fetch("https://example.com")
    let result = t.get()!
    print(result)
}
```

The compiler infers that `fetch` is fallible and therefore `t.get()` is fallible. You handle it with `!` (propagate) or `catch` (handle locally), exactly like any other fallible call. No special error handling model for concurrent code.

### Sleep and Timeouts

`sleep(ms)` pauses the current thread. Combined with `.get_timeout(ms)`, you can bound how long to wait:

```
fn slow_work() int {
    sleep(5000)
    return 42
}

fn main() {
    let t = spawn slow_work()
    let result = t.get_timeout(1000) catch -1
    print(result)
}
```

## Channels

Channels are typed conduits for sending values between concurrent tasks. One end sends, the other receives.

### Creating Channels

```
let (tx, rx) = chan<int>(10)
```

`chan<T>(capacity)` returns a `(Sender<T>, Receiver<T>)` pair. The capacity sets the buffer size -- `send` blocks only when the buffer is full. `chan<T>()` with no argument defaults to capacity 1. This is the only destructuring form in Pluto -- it is specific to `chan`, not a general tuple system.

### Sending and Receiving

```
fn produce(tx: Sender<string>) {
    tx.send("hello")!
    tx.send("world")!
    tx.close()
}

fn consume(rx: Receiver<string>) {
    for msg in rx {
        print(msg)
    }
}

fn main() {
    let (tx, rx) = chan<string>(5)
    let p = spawn produce(tx)
    let c = spawn consume(rx)
    p.get()
    c.get()
}
```

`.send()` blocks until there is space. `.recv()` blocks until a value arrives. Both raise `ChannelClosed` when the channel is closed. `for-in` over a receiver drains it until closed -- this is the idiomatic consumption pattern.

For non-blocking operations, use `try_send` and `try_recv`:

```
tx.try_send(value)!      // raises ChannelFull if buffer is at capacity
let val = rx.try_recv()!  // raises ChannelEmpty if nothing available
```

### Closing and Errors

`tx.close()` signals that no more values will be sent. Further sends raise `ChannelClosed`. Receivers drain buffered values first, then receive `ChannelClosed`. Three built-in error types cover all failure modes:

- `ChannelClosed` -- channel was closed
- `ChannelFull` -- non-blocking send on a full buffer
- `ChannelEmpty` -- non-blocking recv on an empty buffer

These integrate with Pluto's error system. The compiler infers them automatically -- if your function calls `tx.send(v)!`, the compiler knows it can raise `ChannelClosed`.

### Fan-In and Timeouts

Multiple senders can write to the same channel (fan-in). Timed operations bound how long a send or recv blocks:

```
tx.send_timeout(value, 1000)!   // block up to 1 second
let val = rx.recv_timeout(500)!  // block up to 500ms
```

### Select

When you need to wait on multiple channels simultaneously, `select` provides multiplexing:

```
select {
    val = rx1.recv() {
        print("got int: {val}")
    }
    msg = rx2.recv() {
        print("got string: {msg}")
    }
    default {
        print("nothing ready")
    }
}
```

`select` evaluates whichever arm becomes ready first. The `default` arm executes if no channel is immediately ready. Without `default`, `select` blocks until at least one arm can proceed. If all channels are closed and no default is present, `select` raises `ChannelClosed`.

## Inferred Synchronization

> **Status: Designed.** The inferred synchronization model is fully designed but not yet implemented. Today, shared mutable state across tasks is the programmer's responsibility.

Tasks and channels handle most concurrency needs. But real backend systems also need shared state: a session cache, a rate limiter, configuration that updates at runtime.

Go solves this with `sync.Mutex` and discipline. Rust solves it with `Arc<RwLock<T>>`. Java solves it with `synchronized`. Pluto's approach: the compiler already knows everything it needs to solve this automatically.

### The Motivating Example

A service registry that syncs with discovery and serves lookups to concurrent handlers:

```
class ServiceRegistry[discovery: DiscoveryClient] {
    services: Map<string, ServiceEndpoint>

    fn lookup(self, name: string) ServiceEndpoint? {
        return self.services[name]
    }

    fn refresh(mut self) {
        let latest = self.discovery.fetch_all()!
        self.services = latest
    }

    fn start_sync(mut self) {
        while true {
            self.refresh() catch err {
                print("sync failed: {err}")
            }
            sleep(30000)
        }
    }
}

app MyApp[registry: ServiceRegistry, handler: RequestHandler] {
    fn main(self) {
        let sync = spawn self.registry.start_sync()
        sync.detach()

        for conn in listen(8080) {
            spawn self.handler.handle(conn)
        }
    }
}
```

No `shared` keyword, no lock annotations. The compiler sees that `ServiceRegistry` is a DI singleton, `lookup()` is a `self` method called from request handler tasks, and `start_sync()` is a `mut self` method in a background task. It auto-wraps `self` methods with reader locks and `mut self` methods with writer locks. Hundreds of concurrent lookups proceed unblocked. The 30-second refresh briefly takes exclusive access.

### How It Works

**`mut self` is the universal lever.** `self` methods are read-only -- safe to call concurrently. `mut self` methods modify state -- they require exclusive access. The compiler performs whole-program analysis:

1. Walk the DI graph and spawn sites.
2. Identify which singletons are reachable from concurrent contexts.
3. For each concurrently-accessed singleton, inject synchronization: reader locks on `self` methods, writer locks on `mut self` methods.

If a singleton is only accessed from one thread, no locks are generated. Zero overhead when concurrency is absent.

Compare what you write in Go -- `sync.RWMutex` field, `RLock()`/`RUnlock()` in every reader, `Lock()`/`Unlock()` in every writer -- or Java -- `ReentrantReadWriteLock`, `try`/`finally` blocks wrapping every access. In Pluto, you write `fn lookup(self)` and `fn refresh(mut self)`. The compiler handles the rest.

### Channels vs Shared State

| | Channels | DI Singletons (auto-synchronized) |
|---|---------|----------------------------------|
| **Model** | Message passing | Shared memory with auto-locking |
| **Best for** | Pipelines, streaming, producer-consumer | Caches, config, counters, registries |

Use channels when data flows in one direction. Use DI singletons when multiple tasks need to read and occasionally write the same shared state.

### Copy on Spawn

> **Status: Designed.** Not yet implemented.

When you `spawn func(args)`, every argument will be **deep-copied** into the spawned task. The task gets its own independent world:

```
let mut data = [1, 2, 3]
let task = spawn process(data)   // data is deep-copied
data.push(4)                     // caller's copy, unaffected
let result = task.get()!         // task worked on its own copy
```

This eliminates data races by construction. Exceptions: channels are shared by reference (that is the point), DI singletons are shared and auto-synchronized, and strings are pointer-copied because they are immutable.

### Structured Concurrency

> **Status: Designed.** Not yet implemented.

`Task<T>` will be must-use. A task handle must be consumed via `.get()` or `.detach()`. Dropping a handle is a compile error:

```
spawn work()                     // COMPILE ERROR: task handle dropped

let task = spawn fire_and_forget()
task.detach()                    // OK: explicitly detached
```

Cancellation is cooperative. `.cancel()` sets a flag; the task checks at I/O, channel operations, and explicit checkpoints:

```
let task = spawn long_work()
task.cancel()
task.get() catch {
    TaskCancelled => print("cancelled")
}
```

## The Full Picture

Pluto's concurrency model is layered:

1. **`spawn` + `Task<T>`** -- lightweight concurrency with error propagation. Available today.
2. **Channels** -- typed, directional communication between tasks. Available today.
3. **Copy-on-spawn** (designed) -- deep-copy arguments to eliminate data races.
4. **`mut self` enforcement** (designed) -- compiler distinguishes reads from writes.
5. **Inferred synchronization** (designed) -- compiler auto-wraps DI singletons with locks.
6. **Distributed replication** (future) -- same `mut self` distinction extends to cross-pod state.

The goal is that adding concurrency to a Pluto program never requires restructuring your code. You do not wrap types in `Arc<Mutex<T>>`, you do not add `synchronized` blocks, you do not sprinkle `sync.RWMutex` fields into structs. You mark which methods mutate with `mut self`, and the compiler handles the rest.
