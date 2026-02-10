# Concurrency

Pluto supports lightweight concurrency with `spawn` and `Task`. You can run functions on separate threads and collect their results later.

## Spawning Tasks

Use `spawn` to run a function call on a new thread. It returns a `Task<T>` where `T` is the function's return type:

```
fn fib(n: int) int {
    if n <= 1 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}

fn main() {
    let t = spawn fib(30)
    // t has type Task<int>
    // fib(30) is running on another thread
}
```

`spawn` takes a direct function call -- you cannot spawn method calls or closures.

## Getting Results

Call `.get()` on a task to block until the result is ready:

```
fn main() {
    let t1 = spawn fib(30)
    let t2 = spawn fib(25)

    // Both are computing concurrently.
    // .get() blocks until the result is available.
    let r1 = t1.get()
    let r2 = t2.get()

    print("fib(30) = {r1}")
    print("fib(25) = {r2}")
}
```

You can call `.get()` multiple times -- after the first call returns, subsequent calls return the cached result immediately.

## Void Tasks

If the spawned function returns `void`, the task has type `Task<void>`. Call `.get()` to wait for completion:

```
fn do_work() {
    let x = fib(20)
    print("computed: {x}")
}

fn main() {
    let t = spawn do_work()
    // do other stuff...
    t.get()  // wait for it to finish
    print("done")
}
```

## Error Handling

If a spawned function raises an error, the error is captured by the task. It surfaces when you call `.get()`:

```
error ComputeError {
    message: string
}

fn checked_fib(n: int) int {
    if n < 0 {
        raise ComputeError { message: "negative input" }
    }
    return fib(n)
}

fn main() {
    let t = spawn checked_fib(-1)

    // .get() propagates the error -- use catch or !
    let result = t.get() catch -1
    print(result)   // -1
}
```

The same error handling rules apply: use `catch` to handle the error, or `!` to propagate it to your caller.

## Multiple Tasks

Spawn as many tasks as you need. They all run concurrently:

```
fn main() {
    let tasks: [Task<int>] = []
    for i in 0..5 {
        let t = spawn fib(20 + i)
        tasks = tasks + [t]
    }

    // Collect all results
    for t in tasks {
        print(t.get())
    }
}
```

## Limitations

- `spawn` only works with direct function calls (`spawn foo(args)`), not method calls or closures
- There is no `.cancel()` or `.detach()` yet
- Spawned functions share the heap -- there are no move semantics, so concurrent mutation of shared data is the programmer's responsibility
- The garbage collector is paused while tasks are running (up to a 1GB heap ceiling)
