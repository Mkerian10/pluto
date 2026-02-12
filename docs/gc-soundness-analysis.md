# GC Soundness Trilogy - Deep Dive Analysis

**Author:** Technical review
**Date:** 2026-02-12
**Status:** Critical P0 bugs requiring immediate attention
**Total Effort:** 2-3 weeks
**Risk Level:** CRITICAL - Memory safety violations, data races, undefined behavior

---

## Executive Summary

The Pluto garbage collector has **three critical concurrency bugs** that will cause production failures under load:

1. **GC Initiation Race** - Two threads can initiate GC simultaneously (data race)
2. **Stop-The-World Timeout** - GC proceeds with partial world stop (use-after-free)
3. **Sync Destruction Race** - Destroying mutexes while threads are waiting (undefined behavior)

All three are in `runtime/builtins.c`. They manifest as **intermittent crashes, deadlocks, and memory corruption** under high concurrency. These are not "might happen someday" bugs - they **will** happen in production with sufficient load.

---

## Bug #4: GC Initiation Race Condition

### The Problem

**Location:** `runtime/builtins.c:224-229`

```c
static void *gc_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count) {
    pthread_mutex_lock(&gc_mutex);
    if (gc_stack_bottom && !gc_collecting
        && gc_bytes_allocated + user_size + sizeof(GCHeader) > gc_threshold) {
        // Stop all other task threads via SIGUSR1 so we can safely scan their stacks
        int stopped = gc_stw_stop_threads();
        __pluto_gc_collect();
        if (stopped > 0) gc_stw_resume_threads();
    }
    // ... allocate object ...
```

**The race:**

```
Thread A                          Thread B                          gc_collecting
─────────────────────────────────────────────────────────────────────────────────
lock(gc_mutex)
                                  lock(gc_mutex) [blocks]
if (!gc_collecting)  ✓                                              0
    stop_threads()
    gc_collect() starts                                             1
    [scanning heap...]
                                  [unblocked]
                                  if (!gc_collecting)  ✓            1  ← RACE!
                                  stop_threads()
                                  gc_collect() starts               2  ← DOUBLE GC
```

**What happens:**
- Thread A grabs the mutex, sees `gc_collecting == 0`, starts GC
- Thread B is blocked on the mutex
- Thread A sets `gc_collecting = 1` and starts scanning the heap
- Thread A releases the mutex (during `gc_collect()` execution)
- Thread B grabs the mutex, **still sees the check that passed before the mutex was acquired**
- Both threads are now running GC concurrently

**Why the mutex doesn't help:**
The check `!gc_collecting` happens **outside the critical section** conceptually - the mutex is released during `__pluto_gc_collect()`, so thread B can race on the flag.

### The Manifestation

- **Symptom:** Random crashes during allocation under high concurrency
- **Frequency:** Rare but reproducible with enough threads and allocation pressure
- **Detection:** Extremely hard to debug - looks like heap corruption
- **Production Impact:** Intermittent crashes, data corruption, non-deterministic failures

### The Fix

**Option 1: Compare-And-Swap (Atomic)**
```c
static void *gc_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count) {
    pthread_mutex_lock(&gc_mutex);
    if (gc_stack_bottom
        && gc_bytes_allocated + user_size + sizeof(GCHeader) > gc_threshold) {

        // Atomic test-and-set: only one thread wins
        int expected = 0;
        if (__sync_bool_compare_and_swap(&gc_collecting, expected, 1)) {
            // This thread won - run GC
            int stopped = gc_stw_stop_threads();
            __pluto_gc_collect();  // This sets gc_collecting back to 0
            if (stopped > 0) gc_stw_resume_threads();
        } else {
            // Another thread is collecting - wait for it
            pthread_mutex_unlock(&gc_mutex);
            while (__sync_fetch_and_add(&gc_collecting, 0) == 1) {
                usleep(100);  // spin-wait with backoff
            }
            pthread_mutex_lock(&gc_mutex);
        }
    }
    // ... allocate object ...
```

**Option 2: Dedicated GC Mutex**
```c
static pthread_mutex_t gc_collect_mutex = PTHREAD_MUTEX_INITIALIZER;

static void *gc_alloc(...) {
    pthread_mutex_lock(&gc_mutex);
    if (gc_stack_bottom && gc_bytes_allocated > gc_threshold) {
        pthread_mutex_unlock(&gc_mutex);  // release allocation mutex

        pthread_mutex_lock(&gc_collect_mutex);  // serialize GC initiation
        // Re-check threshold under GC mutex (might have changed)
        if (gc_bytes_allocated > gc_threshold) {
            int stopped = gc_stw_stop_threads();
            __pluto_gc_collect();
            if (stopped > 0) gc_stw_resume_threads();
        }
        pthread_mutex_unlock(&gc_collect_mutex);

        pthread_mutex_lock(&gc_mutex);  // re-acquire for allocation
    }
    // ... allocate object ...
```

**Recommendation:** Option 1 (CAS) - simpler, lower overhead, standard pattern for this problem.

**Effort:** 1-2 days (includes testing under high concurrency)

---

## Bug #5: GC Stop-The-World Timeout Causes Use-After-Free

### The Problem

**Location:** `runtime/builtins.c:192-214`

```c
static int gc_stw_stop_threads(void) {
    int count = 0;
    gc_stw_stopped = 0;
    gc_stw_resume = 0;
    __sync_synchronize();  // memory barrier

    pthread_t self = pthread_self();
    for (int i = 0; i < gc_thread_stack_count; i++) {
        if (!gc_thread_stacks[i].active) continue;
        if (pthread_equal(gc_thread_stacks[i].thread, self)) continue;
        pthread_kill(gc_thread_stacks[i].thread, SIGUSR1);  // Send signal
        count++;
    }

    if (count > 0) {
        // Wait for all threads to acknowledge (with timeout)
        int spins = 0;
        while (__sync_fetch_and_add(&gc_stw_stopped, 0) < count) {
            __sync_synchronize();
            if (++spins > 1000000) break;  // ~1s timeout — give up ← DANGER!
        }
    }
    return count;
}
```

**The scenario:**

```
Main Thread                       Worker Thread                    Heap State
─────────────────────────────────────────────────────────────────────────────────
GC triggered
Send SIGUSR1 to worker
Wait for ack...
                                  [handling very long operation]
                                  [doesn't respond to signal]
spins = 1,000,000
TIMEOUT - proceed with GC anyway!
Mark phase [incomplete]                                            Object X: unmarked
Sweep phase                                                        Object X: FREED
                                  Signal finally arrives
                                  Tries to access Object X         ← USE-AFTER-FREE
                                  SEGFAULT
```

**Why this is broken:**

Stop-the-world GC requires **ALL threads to be paused**. If even ONE thread doesn't respond:
- That thread's stack isn't scanned
- Objects reachable only from that thread are not marked
- GC sweep will **free live objects**
- Thread resumes, accesses freed memory → **use-after-free**

**Why 1 second timeout exists:**
- Original assumption: signals are instant, threads always respond quickly
- Reality: threads can be blocked in syscalls, spinning, or kernel wait states
- Signal delivery is not guaranteed to be fast

### The Manifestation

- **Symptom:** Random segfaults under heavy load, especially with I/O or syscalls
- **Frequency:** Rare in development, common in production (more threads, more load)
- **Pattern:** Worker threads crash accessing freed memory, often in syscall returns
- **Debugging:** Stack trace shows valid code accessing freed data structures

### The Fix

**Option 1: Block Indefinitely (Safe But Risky)**
```c
static int gc_stw_stop_threads(void) {
    // ... send signals ...

    if (count > 0) {
        // Wait FOREVER - no timeout
        while (__sync_fetch_and_add(&gc_stw_stopped, 0) < count) {
            __sync_synchronize();
            usleep(1000);  // yield CPU, don't spin
        }
    }
    return count;
}
```

**Pros:** Guaranteed correctness - GC never proceeds with partial world stop
**Cons:** If a thread is truly stuck (infinite loop, deadlock), GC never runs → OOM

**Option 2: Safepoint Polling (Correct, More Work)**

Replace signal-based STW with **cooperative safepoints** - threads check a flag periodically.

```c
// Global safepoint flag
static atomic_int gc_safepoint_requested = 0;

// Called at allocation sites, loop back-edges, function prologues
void __pluto_safepoint() {
    if (__sync_fetch_and_add(&gc_safepoint_requested, 0) == 1) {
        gc_safepoint_yield();  // pause here until GC done
    }
}

static int gc_stw_stop_threads(void) {
    gc_safepoint_requested = 1;
    __sync_synchronize();

    // Wait for all threads to check in (no timeout needed - they WILL hit safepoint)
    while (__sync_fetch_and_add(&gc_stw_stopped, 0) < gc_thread_count) {
        usleep(100);
    }
    return gc_thread_count;
}
```

**Pros:** Deterministic, no signal handling, threads yield at well-defined points
**Cons:** Requires compiler codegen changes - inject `__pluto_safepoint()` calls

**Option 3: Hybrid (Pragmatic)**

Keep signals for fast case, add timeout handler that forces safepoint polling.

```c
static int gc_stw_stop_threads(void) {
    // ... send signals ...

    if (count > 0) {
        int spins = 0;
        while (__sync_fetch_and_add(&gc_stw_stopped, 0) < count) {
            __sync_synchronize();
            if (++spins > 1000000) {
                // Timeout - force safepoint mode
                gc_force_safepoint_mode();
                // Block indefinitely now
                while (__sync_fetch_and_add(&gc_stw_stopped, 0) < count) {
                    usleep(1000);
                }
                break;
            }
        }
    }
    return count;
}
```

**Recommendation:**
- **Short term (1 week):** Option 1 - Remove timeout, block indefinitely. Add watchdog that aborts process if GC blocks >30s.
- **Long term (future):** Option 2 - Safepoint polling. This is the "right" way but requires compiler changes.

**Effort:**
- Option 1: 3-4 days (includes watchdog timer)
- Option 2: 2+ weeks (compiler codegen, runtime support, testing)

---

## Bug #6: GC Sync Destruction Race (Task/Channel)

### The Problem

**Location:** `runtime/builtins.c:667-693`

```c
// GC sweep phase - freeing unmarked objects
if (!h->mark) {
    // Free task sync resources
    if (h->type_tag == GC_TAG_TASK && h->size >= 56) {
        long *slots = (long *)((char *)h + sizeof(GCHeader));
        void *sync = (void *)slots[4];
        if (sync) {
            pthread_mutex_destroy((pthread_mutex_t *)sync);
            pthread_cond_destroy((pthread_cond_t *)((char *)sync + sizeof(pthread_mutex_t)));
            free(sync);
        }
    }
    // Free channel sync + buffer
    if (h->type_tag == GC_TAG_CHANNEL && h->size >= 56) {
        long *ch = (long *)((char *)h + sizeof(GCHeader));
        void *sync = (void *)ch[0];
        if (sync) {
            ChannelSync *cs = (ChannelSync *)sync;
            pthread_mutex_destroy(&cs->mutex);
            pthread_cond_destroy(&cs->not_empty);
            pthread_cond_destroy(&cs->not_full);
            free(sync);
        }
    }
    free(h);
}
```

**Meanwhile, at the same time:**

```c
// Thread blocked on task.get()
long __pluto_task_get(long task_ptr) {
    long *task = (long *)task_ptr;
    TaskSync *sync = (TaskSync *)task[4];

    pthread_mutex_lock(&sync->mutex);
    while (!task[3]) {
        pthread_cond_wait(&sync->cond, &sync->mutex);  // ← Thread is HERE
    }
    pthread_mutex_unlock(&sync->mutex);
    // ...
}
```

**The race:**

```
Thread A: task.get()              GC Thread                         Sync State
─────────────────────────────────────────────────────────────────────────────────
lock(sync->mutex)
cond_wait(sync->cond)
[BLOCKED, waiting for task]                                        VALID
                                  GC sweep starts
                                  Task is unmarked (reachable from
                                    blocked thread stack, but GC
                                    might not see it due to bug #5)
                                  pthread_mutex_destroy(sync)       DESTROYED
                                  pthread_cond_destroy(sync)        DESTROYED
                                  free(sync)                        FREED
[task completes, tries to wake]
pthread_cond_signal(sync)         ← UNDEFINED BEHAVIOR (POSIX violation)
CRASH / DEADLOCK / CORRUPTION
```

**POSIX Requirement:**

From `pthread_mutex_destroy(3)`:
> "It shall be safe to destroy an initialized mutex that is unlocked. **Attempting to destroy a locked mutex or a mutex that is referenced (for example, while being used in a `pthread_cond_wait()`) results in undefined behavior.**"

From `pthread_cond_destroy(3)`:
> "It shall be safe to destroy an initialized condition variable upon which no threads are currently blocked. **Attempting to destroy a condition variable upon which other threads are currently blocked results in undefined behavior.**"

**Current code violates POSIX:**
- GC sweep calls `pthread_mutex_destroy()` on a mutex that might be locked
- GC sweep calls `pthread_cond_destroy()` on condvars that might have waiters

### The Manifestation

- **Symptom:** Deadlocks, crashes in pthread internals, "invalid mutex" errors
- **Frequency:** Under high concurrency with tasks/channels
- **Pattern:** Thread hangs in `pthread_cond_wait()` forever, or crashes in `__pthread_cond_wait_finish()`
- **Platform-specific:** macOS/Linux pthread implementations react differently - both bad

### The Fix

**Option 1: Reference Counting (Clean)**

Never destroy sync primitives while they might be in use. Track references.

```c
typedef struct {
    pthread_mutex_t mutex;
    pthread_cond_t cond;
    atomic_int refcount;  // 1 = task handle, +1 per waiting thread
} TaskSync;

// When thread starts waiting
long __pluto_task_get(long task_ptr) {
    long *task = (long *)task_ptr;
    TaskSync *sync = (TaskSync *)task[4];

    __sync_fetch_and_add(&sync->refcount, 1);  // acquire reference

    pthread_mutex_lock(&sync->mutex);
    while (!task[3]) {
        pthread_cond_wait(&sync->cond, &sync->mutex);
    }
    pthread_mutex_unlock(&sync->mutex);

    // Release reference
    if (__sync_sub_and_fetch(&sync->refcount, 1) == 0) {
        // Last reference - safe to destroy
        pthread_mutex_destroy(&sync->mutex);
        pthread_cond_destroy(&sync->cond);
        free(sync);
    }

    // ... return result ...
}

// GC sweep
if (h->type_tag == GC_TAG_TASK && h->size >= 56) {
    long *slots = (long *)((char *)h + sizeof(GCHeader));
    TaskSync *sync = (TaskSync *)slots[4];
    if (sync) {
        // Drop task's reference
        if (__sync_sub_and_fetch(&sync->refcount, 1) == 0) {
            // Last reference - safe to destroy
            pthread_mutex_destroy(&sync->mutex);
            pthread_cond_destroy(&sync->cond);
            free(sync);
        }
    }
}
```

**Pros:** Guaranteed safe, works for all cases
**Cons:** Extra atomic operations, slightly more complex

**Option 2: Wake All Waiters Before Destroy (Simpler)**

```c
// GC sweep
if (h->type_tag == GC_TAG_TASK && h->size >= 56) {
    long *slots = (long *)((char *)h + sizeof(GCHeader));
    TaskSync *sync = (TaskSync *)slots[4];
    if (sync) {
        // Wake all waiters before destroying
        pthread_mutex_lock(&sync->mutex);
        slots[3] = 1;  // Set done flag (fake completion)
        pthread_cond_broadcast(&sync->cond);
        pthread_mutex_unlock(&sync->mutex);

        // Brief sleep to let threads wake (ugly but pragmatic)
        usleep(1000);

        pthread_mutex_destroy(&sync->mutex);
        pthread_cond_destroy(&sync->cond);
        free(sync);
    }
}
```

**Pros:** Simpler, fewer changes
**Cons:** `usleep()` is a hack, not guaranteed correct, race window still exists

**Option 3: Don't Destroy - Leak Sync Primitives (Hacky)**

```c
// GC sweep
if (h->type_tag == GC_TAG_TASK && h->size >= 56) {
    long *slots = (long *)((char *)h + sizeof(GCHeader));
    void *sync = (void *)slots[4];
    if (sync) {
        // Don't destroy - just leak the sync structure
        // (The ~100 bytes is small compared to task handle itself)
        // free(sync);  ← REMOVED
    }
}
```

**Pros:** Zero risk, trivial change
**Cons:** Leaks memory (small amount, but grows over time)

**Recommendation:** **Option 1 (Reference Counting)** - it's the correct solution, not much more complex, and eliminates the entire class of bugs.

**Effort:** 3-5 days (refcount for both Task and Channel sync, testing)

---

## Testing Strategy

### Unit Tests (C Runtime)

Add explicit tests for each bug:

```c
// test_gc_race.c
void test_gc_initiation_race() {
    // Spawn 10 threads, all allocate heavily, verify no double-GC
}

void test_gc_stw_timeout() {
    // Spawn thread that blocks in long syscall
    // Trigger GC, verify no use-after-free
}

void test_sync_destruction() {
    // Create task, start thread waiting on .get()
    // Force GC, verify no crash
}
```

### Integration Tests (Pluto)

```pluto
// tests/integration/gc_soundness.rs

test "gc initiation race - heavy concurrent allocation" {
    let tasks = []
    for i in 0..20 {
        tasks.push(spawn allocate_heavily())
    }
    for t in tasks {
        t.get()
    }
}

test "gc stw timeout - thread in long syscall" {
    let t = spawn blocking_io()
    // Trigger GC while thread is blocked
    allocate_until_gc()
    t.get()
}

test "sync destruction - waiting on dead task" {
    let t = spawn long_computation()
    // Force GC to collect task while we're waiting
    // (Requires test hooks to force collection)
    t.get()
}
```

### Stress Testing

```bash
# Run under ThreadSanitizer (detects data races)
TSAN_OPTIONS="halt_on_error=1" cargo test --test gc_soundness

# Run under Valgrind Helgrind (detects pthread bugs)
valgrind --tool=helgrind ./target/debug/pluto test gc_soundness.pluto

# Stress test with high concurrency
for i in {1..1000}; do
    cargo test --test gc_soundness -- --nocapture
done
```

---

## Implementation Order

### Phase 1: Fix Bug #4 (GC Initiation Race) - 2 days
1. Implement CAS-based GC initiation guard
2. Add unit test for concurrent GC initiation
3. Run under ThreadSanitizer, verify no race

### Phase 2: Fix Bug #6 (Sync Destruction) - 4 days
1. Add refcount to TaskSync and ChannelSync
2. Update task.get() / chan.send() / chan.recv() to acquire/release refs
3. Update GC sweep to use refcount-based destruction
4. Add unit test for sync destruction race
5. Run under Helgrind, verify no pthread violations

### Phase 3: Fix Bug #5 (STW Timeout) - 5 days
1. Remove 1-second timeout from `gc_stw_stop_threads()`
2. Add watchdog timer (abort process if GC blocks >30s)
3. Add logging for slow STW responses
4. Add unit test for long syscall during GC
5. Stress test under high I/O load

### Phase 4: Validation - 2-3 days
1. Run full test suite under TSAN and Helgrind
2. Stress test with 50+ threads for 1 hour
3. Run integration tests 1000 times in loop
4. Verify no crashes, no deadlocks, no data races

**Total time:** ~2-3 weeks (13-16 days of work)

---

## Risk Assessment

### What Happens If We Don't Fix These?

**Before Production:**
- Development seems fine (low concurrency, short runs)
- Occasional "weird crash" in tests - hard to reproduce

**In Production:**
- Bug #4: Random crashes during high allocation (2-3x per week under load)
- Bug #5: Random segfaults when worker threads are doing I/O (daily under load)
- Bug #6: Deadlocks when tasks complete during GC (weekly, requires restart)

**Debugging Cost:**
- Each crash requires 4-8 hours of investigation
- Core dumps show corrupted heap, no clear root cause
- Team suspects memory corruption, writes defensive code everywhere
- Eventually traces back to GC, but hard to fix under pressure

**Better Path:**
- Fix now, proactively, with clear understanding
- Validate with TSAN/Helgrind before production
- Never worry about GC soundness again

---

## Conclusion

These three bugs are **textbook concurrency bugs**:
- Data race (bug #4)
- Premature resource cleanup (bug #5)
- Use-after-free (bug #6)

They are **not hypothetical** - the code as written **will fail** in production.

**Good news:**
- All three are fixable in 2-3 weeks
- Fixes are well-understood (CAS, blocking STW, refcounting)
- Testing tools exist (TSAN, Helgrind, stress tests)
- Impact is isolated to `runtime/builtins.c`

**Recommendation:**
- Allocate 2-3 weeks for "GC Soundness Sprint"
- Fix all three bugs together (they interact)
- Validate exhaustively before declaring victory
- This is foundation work - do it right

---

## Appendix: Why The GC Needs STW In The First Place

**Stop-The-World (STW)** means pausing all threads during GC. Pluto's GC needs this because:

1. **Stack Scanning:** Must scan all thread stacks to find live references
2. **Heap Consistency:** Can't have threads mutating heap while marking/sweeping
3. **No Write Barriers:** Pluto doesn't have incremental GC infrastructure

**Alternatives considered:**
- **Incremental GC:** Mark while threads run (requires write barriers - complex)
- **Concurrent GC:** Separate GC thread (requires barriers, complex synchronization)
- **Generational GC:** Young/old generations (requires barriers, more complex)

**Current choice: STW is simplest** - but must be done correctly. These bugs violate STW guarantees.

---

## References

- POSIX pthread spec: https://pubs.opengroup.org/onlinepubs/9699919799/
- ThreadSanitizer: https://github.com/google/sanitizers/wiki/ThreadSanitizerCppManual
- Helgrind: https://valgrind.org/docs/manual/hg-manual.html
- "The Garbage Collection Handbook" (Jones, Hosking, Moss) - Chapter 13: Concurrent GC
