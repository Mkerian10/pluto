# Testing Concurrent Code

Tests that use `spawn`, channels, or `select` run under a deterministic test scheduler. In test mode there are no OS threads: spawned tasks become cooperative fibers, and the scheduler decides exactly which task runs at every blocking point. The same test run always executes the same way — concurrency bugs reproduce instead of flaking.

The scheduler has four strategies. Which one a test file uses is chosen with a `tests` block:

```
tests[scheduler: Exhaustive] {
    test "name" {
        // test body
    }
}
```

A file can have at most one `tests` block, and its strategy applies to every test in the file (including bare `test` blocks outside it). Without a `tests` block, tests run under `Sequential`.

## The Four Strategies

| Strategy | What it does |
|---|---|
| `Sequential` (default) | `spawn` runs the task inline, to completion, at the spawn site. No interleaving at all. |
| `RoundRobin` | Tasks are fibers. At each blocking point, control passes to the next ready fiber in creation order. One deterministic concurrent interleaving. |
| `Random` | At each blocking point, a random ready fiber runs. The test is repeated 100 times with different seeds to sample the schedule space. |
| `Exhaustive` | Systematically runs *every meaningfully distinct interleaving* of the test and requires all of them to pass. |

### Sequential

The default. A spawned task runs immediately and completely at the `spawn` expression, as if it were a plain function call. This is fast and perfectly deterministic, and it is all you need when tasks don't coordinate with each other:

```
fn add(a: int, b: int) int {
    return a + b
}

test "spawn returns result" {
    let t = spawn add(1, 2)
    expect(t.get()).to_equal(3)
}
```

What Sequential cannot do is interleave. If a spawned task has to *wait* for something the test body does later, there is nothing to resume it — the task runs before the test body continues. That is the one failure mode to know about, covered in the next section.

### RoundRobin

The cheapest strategy with real concurrency. Every task is a fiber; whenever a fiber blocks (channel send/recv, `select`, `task.get()`), the scheduler hands control to the next ready fiber in creation order. Coordinating tasks make progress together, and the interleaving is still fully deterministic:

```
fn pong(ping_rx: Receiver<int>, pong_tx: Sender<int>) {
    let v = ping_rx.recv()!
    pong_tx.send(v + 1)!
}

tests[scheduler: RoundRobin] {
    test "ping pong" {
        let (ping_tx, ping_rx) = chan<int>(1)
        let (pong_tx, pong_rx) = chan<int>(1)
        let t = spawn pong(ping_rx, pong_tx)
        ping_tx.send(1)!
        expect(pong_rx.recv()!).to_equal(2)
        t.get()!
    }
}
```

RoundRobin checks that your code works under *one* concurrent schedule. It will not find bugs that only appear under a different ordering.

### Random

Like RoundRobin, but at each blocking point the scheduler picks a random ready fiber, and the whole test is run 100 times with different seeds. Good for shaking out ordering assumptions cheaply:

```
tests[scheduler: Random] {
    test "transfer completes under random schedules" {
        ...
    }
}
```

When a run fails, the runner prints the seed and iteration so the exact schedule can be replayed. Two environment variables control the sampling: `PLUTO_TEST_SEED` fixes the base seed, `PLUTO_TEST_ITERATIONS` changes the run count.

### Exhaustive

The strongest strategy: the test is run repeatedly, once per *meaningfully distinct* interleaving, until the entire schedule space is covered — and every interleaving must pass. Naively there are exponentially many schedules, so the scheduler uses **dynamic partial order reduction** (DPOR): two schedules that differ only in the order of operations that cannot affect each other (different channels, no shared state) produce the same result, so only one of them is run. What remains is every ordering that could actually change behavior.

```
fn produce(tx: Sender<int>, value: int) {
    tx.send(value)!
}

tests[scheduler: Exhaustive] {
    test "two producers race" {
        let (tx, rx) = chan<int>(2)
        let a = spawn produce(tx, 1)
        let b = spawn produce(tx, 2)
        let first = rx.recv()!
        let second = rx.recv()!
        a.get()!
        b.get()!
        expect(first + second).to_equal(3)
    }
}
```

The two producers race to send first, so `first` could be `1` or `2` — the assertion only checks the order-independent sum. The runner reports its coverage of the schedule space:

```
test two producers race ...   Exhaustive: 4 schedules explored
ok
```

If an assertion fails or a deadlock exists in *any* interleaving, Exhaustive finds it and reports which schedule triggered it. Exploration is capped at 10,000 schedules by default; `PLUTO_MAX_SCHEDULES` and `PLUTO_MAX_DEPTH` adjust the limits for very large tests.

## "Blocked ... in Sequential Test Mode"

The first concurrent test most people write looks like this: spawn a worker, send it a request, read its reply.

```
fn worker(requests: Receiver<int>, replies: Sender<int>) {
    let n = requests.recv()!
    replies.send(n * 2)!
}

test "worker doubles" {
    let (req_tx, req_rx) = chan<int>(1)
    let (rep_tx, rep_rx) = chan<int>(1)
    let t = spawn worker(req_rx, rep_tx)
    req_tx.send(21)!
    expect(rep_rx.recv()!).to_equal(42)
    t.get()!
}
```

This code is correct under concurrent execution — and it aborts under the default scheduler:

```
pluto: blocked channel recv on empty buffer in sequential test mode — spawned
tasks run inline, so coordinating tasks cannot interleave. If this code is
correct under concurrency, run it under the exploration scheduler:
tests[scheduler: Exhaustive] { ... }. Strategies: Sequential (default),
RoundRobin, Random, Exhaustive.
```

Decoding it: under `Sequential`, the worker runs *inline, at the spawn site*, before `req_tx.send(21)!` ever executes. It calls `requests.recv()` on a still-empty channel, and no other task can run to fill it — so the scheduler stops. **This is a property of the sequential strategy, not a verdict on your program.** The same message appears as `blocked channel send on full buffer` (a task sends into a channel nothing has drained yet) and `blocked select with no ready channels` (no arm can proceed and there is no `default`).

The fix is in the message: if the code is supposed to work because tasks interleave, run it under a scheduler that interleaves them. Wrap the tests in a block:

```
tests[scheduler: Exhaustive] {
    test "worker doubles" {
        let (req_tx, req_rx) = chan<int>(1)
        let (rep_tx, rep_rx) = chan<int>(1)
        let t = spawn worker(req_rx, rep_tx)
        req_tx.send(21)!
        expect(rep_rx.recv()!).to_equal(42)
        t.get()!
    }
}
```

Now the worker fiber blocks on `recv`, the test body resumes and sends, the worker wakes — and Exhaustive proves the test passes under every distinct ordering.

**Rule of thumb:** the moment a test spawns a task that communicates with the test body (or with other tasks) through channels or `select`, put it under `tests[scheduler: Exhaustive]`. Use `RoundRobin` when you only want one deterministic concurrent schedule, and `Random` for cheap broad sampling of bigger tests where exhaustive exploration is too slow.

## Real Deadlocks

Under the exploration schedulers, a deadlock report means the real thing: an interleaving exists in which every task is blocked and none can ever proceed. The runner shows what each fiber was waiting on, and under Exhaustive, which schedule reached the deadlock:

```
pluto: deadlock detected in test
  Fiber 0: blocked on task.get()
  Fiber 1: blocked on chan.recv()
  Fiber 2: blocked on chan.recv()
  Exhaustive: 1 schedule explored
  1 failure found:
    - deadlock in schedule 0 (depth 3)
```

Fiber 0 is the test body itself; fibers are numbered in spawn order. Here two workers each wait to receive from a channel only the other would fill — a circular wait that no schedule can resolve. Unlike the sequential-mode abort above, this one is telling you about a bug in the program.

## Testing a `select`

`select` works under all strategies, but it is most at home under `Exhaustive`, which explores each arm-readiness ordering that can occur:

```
fn echo(inbox: Receiver<int>, outbox: Sender<int>) {
    let v = inbox.recv()!
    outbox.send(v + 1)!
}

tests[scheduler: Exhaustive] {
    test "select hears whichever worker answers" {
        let (a_tx, a_rx) = chan<int>(1)
        let (b_tx, b_rx) = chan<int>(1)
        let (out_tx, out_rx) = chan<int>(2)
        let a = spawn echo(a_rx, out_tx)
        let b = spawn echo(b_rx, out_tx)
        a_tx.send(10)!
        b_tx.send(20)!
        let mut total = 0
        let mut seen = 0
        while seen < 2 {
            select {
                v = out_rx.recv() {
                    total = total + v
                    seen = seen + 1
                }
            }
        }
        a.get()!
        b.get()!
        expect(total).to_equal(32)
    }
}
```

Either worker may answer first; the test asserts only what must hold in every ordering. Exhaustive then runs all distinct orderings and confirms it.

## Pinning a Failure as a Regression Test

Every test run records its scheduling decisions. When anything fails — an
`expect`, a deadlock, an exhaustive-found interleaving — the runner prints a
repro block:

```
FAIL (line 19): expected 2 to equal 1
  strategy: Random  seed: 0x3  iteration: 1
  schedule: ptsched:v1:AQABAgEAAQEBAA==
  rerun:    pluto test <file> --test "transfer" --schedule ptsched:v1:AQABAgEAAQEBAA==
```

The `rerun:` line reproduces the failure immediately. To keep it reproduced
forever, copy the pin into the test header:

```
tests[scheduler: Random, seed: 7, iterations: 1000] {
    // Re-runs exactly that random iteration:
    test "regression: lost transfer, #512" [seed: 3, iteration: 1] { ... }

    // Or replays the recorded interleaving decision by decision:
    test "regression: lost transfer, #512" [schedule: "ptsched:v1:AQABAgEAAQEBAA=="] { ... }
}
```

A seed pin is compact and survives unrelated refactors; a schedule pin is
exact and is the only repro form for failures found by Exhaustive (no seed
generates a DPOR schedule). A replayed schedule that no longer matches the
code's concurrency structure **fails loudly** rather than silently passing —
re-record it with `--until-failure` and update the pin.

The block header also takes `seed:` and `iterations:` to fix Random's
sampling in source, and the CLI can override everything:

```
pluto test file.pt --test "one test"             # filter to one test
pluto test file.pt --strategy Exhaustive         # override the block strategy
pluto test file.pt --seed 0x3                    # hex seeds, as printed
pluto test file.pt --test "t" --until-failure    # hunt seeds until a failure, print its pin
pluto test file.pt --test "t" --schedule ptsched:v1:...   # replay a token
```

CLI flags beat in-source pins, so a stale pin can be re-hunted without
editing the file.
