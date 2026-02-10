# Channels

Channels are Pluto's primitive for communication between concurrent tasks. A channel is a typed, directional conduit: one end sends values, the other receives them.

## Creating Channels

Use `chan<T>()` to create a channel. It returns a sender and receiver pair:

```
let (tx, rx) = chan<int>(10)    // buffered channel with capacity 10
let (tx, rx) = chan<string>()   // default capacity (1)
```

`tx` has type `Sender<int>` and `rx` has type `Receiver<int>`. You send on the sender and receive on the receiver -- the type system prevents mixing them up.

## Sending and Receiving

`.send()` blocks until there is space in the buffer. `.recv()` blocks until a value is available. Both are fallible:

```
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.send(42)!
    let val = rx.recv()!
    print(val)  // 42
}
```

## Channels with Spawn

Channels become powerful when combined with `spawn`. A common pattern is spawning a producer that sends values while the main thread consumes them:

```
fn producer(tx: Sender<int>, count: int) {
    for i in 1..count + 1 {
        tx.send(i)!
    }
    tx.close()
}

fn main() {
    let (tx, rx) = chan<int>(5)
    spawn producer(tx, 10)

    let sum = 0
    for val in rx {
        sum = sum + val
    }
    print(sum)  // 55
}
```

The producer runs on a separate thread, sending values into the channel. The main thread iterates over the receiver with `for-in`, which automatically stops when the channel is closed.

## For-in Iteration

`for-in` on a receiver drains values until the channel is closed:

```
for msg in rx {
    process(msg)
}
// Loop exits when channel is closed and drained
```

`break` and `continue` work normally inside the loop:

```
for val in rx {
    if val == 0 {
        continue
    }
    if val < 0 {
        break
    }
    print(val)
}
```

## Closing Channels

Call `.close()` on the sender to signal that no more values will be sent:

```
tx.close()
```

After closing:
- Further `.send()` calls raise `ChannelClosed`
- `.recv()` drains any buffered values, then raises `ChannelClosed`
- `for-in` loops exit cleanly

Closing is idempotent -- closing an already-closed channel is a no-op.

## Non-Blocking Operations

`.try_send()` and `.try_recv()` return immediately instead of blocking:

```
fn main() {
    let (tx, rx) = chan<int>(2)

    tx.try_send(1)!
    tx.try_send(2)!

    // Buffer is full -- try_send fails with ChannelFull
    tx.try_send(3) catch print("channel full")

    print(rx.try_recv()!)  // 1
    print(rx.try_recv()!)  // 2

    // Buffer is empty -- try_recv fails with ChannelEmpty
    let val = rx.try_recv() catch -1
    print(val)  // -1
}
```

## Error Handling

Channel operations integrate with Pluto's error system. Three built-in error types:

- `ChannelClosed` -- sent/received on a closed channel
- `ChannelFull` -- `try_send()` on a full buffer
- `ChannelEmpty` -- `try_recv()` on an empty buffer

Use `!` to propagate or `catch` to handle:

```
// Propagate
tx.send(value)!

// Catch with fallback value
let val = rx.recv() catch 0

// Catch with handler
let val = rx.recv() catch e { -1 }
```

## Fan-In (Multiple Senders)

Multiple tasks can send to the same channel:

```
fn send_val(tx: Sender<int>, v: int) {
    tx.send(v)!
}

fn main() {
    let (tx, rx) = chan<int>(3)
    spawn send_val(tx, 10)
    spawn send_val(tx, 20)
    spawn send_val(tx, 30)

    let sum = 0
    for i in 0..3 {
        sum = sum + rx.recv()!
    }
    print(sum)  // 60
}
```

## Passing Channels to Functions

Sender and Receiver are first-class types that can be passed as function arguments:

```
fn send_greeting(tx: Sender<string>) {
    tx.send("hello from another function")!
}

fn receive_greeting(rx: Receiver<string>) string {
    return rx.recv()!
}

fn main() {
    let (tx, rx) = chan<string>(1)
    send_greeting(tx)!
    let msg = receive_greeting(rx)!
    print(msg)
}
```

## Limitations

- Channels must be explicitly closed with `.close()` -- there is no automatic close-on-drop yet
- `chan<T>()` with no capacity argument uses capacity 1 (not true rendezvous)
- There is no `select` for waiting on multiple channels simultaneously
- Buffered values are copied by value (pointer copy for heap types) -- no move semantics
