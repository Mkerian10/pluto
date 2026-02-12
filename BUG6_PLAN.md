# Bug #6: TaskSync Destruction Race - Implementation Plan

## The Problem

**Race Condition:** Worker thread accesses TaskSync after GC frees it.

### Scenario
1. Main thread spawns task → TaskSync allocated (`calloc`, mutex/cond initialized)
2. Worker thread starts executing in `__pluto_spawn_trampoline()`
3. **Main thread doesn't call `.get()`** → task becomes unreachable
4. **GC runs** and collects the task object (no references)
5. **GC sweep frees TaskSync** (lines 667-677 in builtins.c):
   ```c
   pthread_mutex_destroy(&sync->mutex);
   pthread_cond_destroy(&sync->cond);
   free(sync);
   ```
6. **Worker thread completes**, tries to access sync at line 3260:
   ```c
   TaskSync *sync = (TaskSync *)task[4];
   pthread_mutex_lock(&sync->mutex);  // 💥 CRASH - mutex destroyed!
   ```
7. **Undefined behavior:** Use-after-free, double-free, or crash

### Root Cause
**TaskSync lifetime is tied to task GC lifetime, but worker thread still references it.**

The task object can be collected while the worker thread is running if:
- User spawns task but never calls `.get()` or `.detach()`
- GC runs before worker completes
- No stack reference to task keeps it alive

## Solution Options

### Option 1: Reference Counting (Recommended)
**Keep TaskSync alive while worker thread is running.**

#### Approach
Add atomic reference count to TaskSync:
```c
typedef struct {
    atomic_int refcount;      // NEW: 1 = task alive, 2 = task + thread alive
    pthread_mutex_t mutex;
    pthread_cond_t cond;
} TaskSync;
```

**Lifecycle:**
1. `__pluto_task_spawn()`: Initialize refcount to 2 (task object + worker thread)
2. Worker thread on exit: Decrement refcount, free if 0
3. GC sweep: Decrement refcount, free if 0

**Pros:**
- Safe - TaskSync only freed when both task and worker are done
- Minimal overhead (one atomic decrement per task)
- Clean separation of concerns

**Cons:**
- Adds 4 bytes to TaskSync
- Requires atomic operations

### Option 2: Keep Task Alive While Worker Runs
**Prevent GC from collecting tasks with running workers.**

#### Approach
Add global weak reference set:
```c
static TaskHandle *running_tasks[GC_MAX_THREAD_STACKS];
static int running_task_count = 0;
```

Worker thread registers task on entry, unregisters on exit. GC marks these as roots.

**Pros:**
- No TaskSync changes needed
- Simpler mental model (task alive = thread alive)

**Cons:**
- Global state to manage
- Requires GC mutex on worker entry/exit
- Potential for memory leaks if implementation buggy

### Option 3: Detach by Default
**Make all tasks detached unless `.get()` is called.**

Worker thread owns TaskSync, frees on completion.

**Pros:**
- No reference counting needed

**Cons:**
- **Breaking change** - changes task semantics
- Still need synchronization for `.get()` callers
- Doesn't fully solve the race (`.get()` after GC still broken)

## Recommended: Option 1 (Reference Counting)

### Implementation Plan

#### Phase 1: Add Reference Counting to TaskSync
**File:** `runtime/builtins.c`

1. **Update TaskSync structure:**
   ```c
   typedef struct {
       atomic_int refcount;  // 1 = task only, 2 = task + worker
       pthread_mutex_t mutex;
       pthread_cond_t cond;
   } TaskSync;
   ```

2. **Create helper functions:**
   ```c
   static TaskSync *tasksync_create(void) {
       TaskSync *sync = (TaskSync *)calloc(1, sizeof(TaskSync));
       pthread_mutex_init(&sync->mutex, NULL);
       pthread_cond_init(&sync->cond, NULL);
       atomic_store(&sync->refcount, 2);  // task + worker
       return sync;
   }

   static void tasksync_release(TaskSync *sync) {
       int old = atomic_fetch_sub(&sync->refcount, 1);
       if (old == 1) {  // Was last reference
           pthread_mutex_destroy(&sync->mutex);
           pthread_cond_destroy(&sync->cond);
           free(sync);
       }
   }
   ```

3. **Update `__pluto_task_spawn()`:**
   ```c
   TaskSync *sync = tasksync_create();  // refcount = 2
   task[4] = (long)sync;
   ```

4. **Update worker thread cleanup:**
   In `__pluto_spawn_trampoline()` at end (line 3290):
   ```c
   pthread_cond_signal(&sync->cond);
   pthread_mutex_unlock(&sync->mutex);
   tasksync_release(sync);  // Worker done - decrement refcount
   ```

5. **Update GC sweep:**
   Replace lines 667-677 with:
   ```c
   if (h->type_tag == GC_TAG_TASK && h->size >= 56) {
       long *slots = (long *)((char *)h + sizeof(GCHeader));
       void *sync = (void *)slots[4];
       if (sync) {
   #ifndef PLUTO_TEST_MODE
           tasksync_release((TaskSync *)sync);  // Task collected - decrement refcount
   #endif
       }
   }
   ```

#### Phase 2: Test Mode Support
**File:** `runtime/builtins.c`

Add no-op reference counting for test mode:
```c
#ifdef PLUTO_TEST_MODE
typedef struct {
    int refcount;  // Non-atomic for test mode
    // no mutex/cond
} TaskSync;

static TaskSync *tasksync_create(void) {
    TaskSync *sync = (TaskSync *)calloc(1, sizeof(TaskSync));
    sync->refcount = 2;
    return sync;
}

static void tasksync_release(TaskSync *sync) {
    sync->refcount--;
    if (sync->refcount == 0) {
        free(sync);
    }
}
#endif
```

#### Phase 3: Testing

1. **Create stress test for task destruction:**
   ```pluto
   // spawn_without_get.pluto
   fn worker() int {
       let sum = 0
       for i in 0..10000 {
           sum = sum + i
       }
       return sum
   }

   fn main() {
       // Spawn 100 tasks but never call .get()
       // Tasks become unreachable immediately
       for i in 0..100 {
           let task = spawn worker()
           // No .get() - task dropped
       }

       // Force GC while workers might still be running
       let arr = []
       for i in 0..1000 {
           arr.push([1,2,3,4,5])
       }

       print("Done")
   }
   ```

2. **Run under different GC thresholds:**
   - 4KB threshold (aggressive GC)
   - Run 1000 iterations
   - Should NOT crash

3. **Verify with concurrency tests:**
   - All existing concurrency tests should pass
   - No regressions in task lifecycle

4. **Leak test:**
   - Verify TaskSync is actually freed (no leaks)
   - Use valgrind/leaksanitizer if available
   - Count allocations vs frees in test wrapper

#### Phase 4: Documentation

Update `docs/design/concurrency.md`:
- Document TaskSync reference counting
- Explain task lifecycle vs worker lifecycle
- Add section on GC safety for concurrent objects

## Validation Criteria

**Must pass:**
- ✅ Stress test (spawn without .get()) runs without crashes
- ✅ All 62 concurrency tests pass
- ✅ No memory leaks (TaskSync properly freed)
- ✅ No regressions in task.get() behavior

**Nice to have:**
- Valgrind clean (if available on macOS)
- Performance benchmarks unchanged

## Effort Estimate
- **Phase 1:** 2-3 hours (reference counting implementation)
- **Phase 2:** 30 minutes (test mode support)
- **Phase 3:** 1-2 hours (testing and validation)
- **Phase 4:** 30 minutes (documentation)

**Total: ~4-6 hours (half day)**

## Risks & Mitigations

**Risk 1:** Atomic operations overhead
- **Mitigation:** Refcount ops only at task spawn/completion (rare)
- **Impact:** Negligible (2 atomic ops per task)

**Risk 2:** Refcount leaks if implementation buggy
- **Mitigation:** Thorough testing, leak detection tests
- **Fallback:** Option 2 (global running task set)

**Risk 3:** Test mode behavior differs from production
- **Mitigation:** Same refcount logic, just non-atomic
- **Verification:** Test mode tests should catch issues

## Alternative Considered: Shared Pointers

Use C11 atomic shared pointers instead of manual refcounting.

**Rejected because:**
- Not available in C99
- More complex than simple refcount
- No significant benefit for this use case

## Next Steps

1. Implement Phase 1 (reference counting)
2. Verify compilation and basic smoke test
3. Create stress test
4. Run full test suite
5. Commit with detailed explanation
6. Update BUGS_AND_FEATURES.md (move #6 to "Recently Fixed")
