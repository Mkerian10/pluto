# std.time

Wall-clock time, monotonic time, and sleep.

```
import std.time
```

## Functions

### now / now_ns

```
time.now() int       // wall-clock milliseconds since Unix epoch
time.now_ns() int    // wall-clock nanoseconds since Unix epoch
```

Wall-clock time. Subject to system clock adjustments. Use for timestamps, not benchmarking.

### monotonic / monotonic_ns

```
time.monotonic() int       // monotonic milliseconds
time.monotonic_ns() int    // monotonic nanoseconds
```

Monotonically increasing clock. Never goes backwards. Use for measuring elapsed time.

### sleep

```
time.sleep(ms: int)
```

Blocks the current thread for at least `ms` milliseconds.

### elapsed

```
time.elapsed(start_ms: int) int
```

Returns `monotonic() - start_ms`. Convenience for timing blocks of code.

## Example: Benchmarking

```
import std.time

fn main() {
    let wall = time.now()
    print("Unix timestamp (ms): {wall}")

    let start = time.monotonic()
    time.sleep(100)
    let ms = time.elapsed(start)
    print("Slept for {ms}ms")
}
```
