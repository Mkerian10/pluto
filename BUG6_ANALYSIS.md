# Bug #6 Analysis: TaskSync Destruction Race (NOT A BUG)

## TL;DR

**Bug #6 does not exist.** The GC correctly scans all worker thread stacks, so task objects remain reachable (and alive) while workers are running.

## Original Hypothesis

Bug #6 was theorized to be a race condition where:
1. Main thread spawns task without calling `.get()`
2. Task handle becomes unreachable from main thread
3. GC runs and frees task object + TaskSync
4. Worker thread tries to access TaskSync → crash (use-after-free)

## Why It Doesn't Exist

The hypothesis missed a critical detail: **worker threads keep the task pointer on their stack**.

### Worker Thread Lifecycle

```c
static void *__pluto_spawn_trampoline(void *arg) {
    long *task = (long *)arg;  // ← Task pointer stored on worker's stack

    // Register this thread's stack with GC
    gc_thread_stacks[my_stack_slot].thread = self;
    gc_thread_stacks[my_stack_slot].stack_lo = stack_lo;
    gc_thread_stacks[my_stack_slot].stack_hi = stack_hi;
    gc_thread_stacks[my_stack_slot].active = 1;

    // Run the task closure
    long result = ((long(*)(long))fn_ptr)(closure_ptr);

    // Access TaskSync to write result
    TaskSync *sync = (TaskSync *)task[4];
    pthread_mutex_lock(&sync->mutex);
    task[1] = result;
    task[3] = 1;  // done
    pthread_cond_signal(&sync->cond);
    pthread_mutex_unlock(&sync->mutex);

    return NULL;
}
```

### GC Root Scanning

When GC runs, it scans all registered thread stacks:

```c
// runtime/builtins.c lines 627-645
// 3c. Scan all OTHER registered thread stacks as additional GC roots.
pthread_t gc_self = pthread_self();
for (int ti = 0; ti < gc_thread_stack_count; ti++) {
    if (!gc_thread_stacks[ti].active) continue;
    if (pthread_equal(gc_thread_stacks[ti].thread, gc_self)) continue;
    void *tlo = gc_thread_stacks[ti].stack_lo;
    void *thi = gc_thread_stacks[ti].stack_hi;

    // Scan entire stack range for pointers
    for (long *p = (long *)tlo; (void *)p < thi; p++) {
        gc_mark_candidate((void *)*p);  // ← Finds task pointer!
    }
}
```

### Reachability Analysis

```
Main thread:     spawn worker() → task handle dropped → unreachable from main
                                  ↓
Worker thread:   long *task = arg → task on stack → REACHABLE from worker
                                  ↓
GC scan:        Scans worker stack → finds task pointer → marks task as live
                                  ↓
Result:         Task object NOT freed → worker can safely access TaskSync
```

## Verification

Attempted to trigger the bug by:
1. Disabling the compiler check that enforces task handle usage
2. Spawning 100 tasks without keeping references
3. Forcing aggressive GC (2KB threshold)
4. Running 5 iterations

**Result:** No crashes. All runs completed successfully.

## Conclusion

The GC implementation is **correct**. It properly scans all thread stacks as GC roots, which prevents premature collection of task objects that workers still reference.

The compiler's "must-use" check for task handles is still valuable as a **lint** (encourages explicit `.get()` or `.detach()`), but it's not preventing a memory safety bug.

## Files Examined

- `runtime/builtins.c` lines 3245-3316 (worker thread lifecycle)
- `runtime/builtins.c` lines 627-645 (GC thread stack scanning)
- `runtime/builtins.c` lines 3269-3277 (thread stack registration)
- `bug6_test/trigger_crash.pluto` (stress test, could not trigger crash)

## Recommendation

**Close Bug #6 as "Not a Bug - Working as Designed"**

The GC correctly handles concurrent task execution. No changes needed to runtime.
