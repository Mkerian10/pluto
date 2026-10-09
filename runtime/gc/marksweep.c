//──────────────────────────────────────────────────────────────────────────────
// Pluto Runtime: Garbage Collector
//
// Memory management and stop-the-world garbage collection.
//
// Design:
// - Conservative mark-and-sweep collector
// - Interval tables for fast pointer lookup
// - Stop-the-world via safepoint polling (production mode)
// - Single-threaded sequential collection (test mode)
// - Supports concurrent task execution with thread stack scanning
//──────────────────────────────────────────────────────────────────────────────

#include "builtins.h"

// ── GC Infrastructure ─────────────────────────────────────────────────────────

// Interval for binary-search pointer lookup
typedef struct { void *start; void *end; GCHeader *header; } GCInterval;
// Array data buffer interval
typedef struct { void *start; void *end; void *array_handle; } GCDataInterval;

// Global GC state
static GCHeader *gc_head = NULL;
static size_t gc_bytes_allocated = 0;

// ── Collection threshold policy ────────────────────────────────────────────────────────────
//
// A collection runs when gc_bytes_allocated exceeds gc_threshold. After each
// collection the threshold becomes
//
//     gc_threshold = max(2 * live_bytes, gc_adaptive_floor)
//
// The 2x-live term is the classic proportional-space policy: a workload
// retaining R bytes collects after allocating about another R, bounding
// floating garbage to O(live).
//
// The floor bounds collection FREQUENCY. Every cycle pays costs that have
// nothing to do with how much it reclaims: the stop-the-world handshake and
// a conservative scan of every registered thread's live stack. With the old
// fixed 256 KiB floor, an allocation-steady workload with ~zero survivors
// ran one full collection per 256 KiB allocated, so those fixed costs were
// paid at a rate set by an arbitrary constant — the #380 collapse. The floor
// therefore adapts:
//
// - Growth trigger (survivor-awareness): a cycle that reclaimed at least
//   half its budget (freed_bytes * 2 >= threshold) is allocation-churn, not
//   retention growth; collecting half as often would roughly halve its
//   per-byte cost, so the floor doubles. A cycle that reclaimed less than
//   half was driven by a growing live set, where the 2x-live term is the
//   right driver, so the floor halves — a transient spike decays back
//   instead of permanently inflating the budget.
//
// - Growth cap (what the budget must amortize): the dominant fixed cost
//   scales with the number of thread stacks scanned, so the cap does too:
//   GC_FLOOR_BASE_CAP + GC_FLOOR_PER_THREAD per registered thread (fibers in
//   test mode), clamped to GC_FLOOR_MAX. A single-threaded program caps at
//   2 MiB — small enough that sweep batches stay cache-friendly — while a
//   1000-idle-task server caps at 64 MiB, amortizing its ~ms-scale scans
//   over proportionally more allocation. 64 KiB of garbage budget per thread
//   is modest next to the 512 KiB of stack each thread already reserves.
//   The cap is recomputed every cycle from the current thread count, so the
//   budget also decays when threads exit.
//
// Worst-case retained-but-dead memory between collections is
// max(2 * live, floor) with floor <= 64 MiB — a deliberate space-for-time
// trade for a backend-server runtime.
#define GC_MIN_THRESHOLD    ((size_t)256 * 1024)
#define GC_FLOOR_BASE_CAP   ((size_t)2 * 1024 * 1024)
#define GC_FLOOR_PER_THREAD ((size_t)64 * 1024)
#define GC_FLOOR_MAX        ((size_t)64 * 1024 * 1024)
static size_t gc_threshold = GC_MIN_THRESHOLD;
static size_t gc_adaptive_floor = GC_MIN_THRESHOLD;
static void *gc_stack_bottom = NULL;

// Observability: PLUTO_GC_LOG=1 prints one line per collection to stderr
// (cycle number, live/freed bytes, next threshold, pause duration).
static long gc_cycle_count = 0;
static int gc_log_enabled = -1;  // -1: getenv not consulted yet
#ifdef PLUTO_TEST_MODE
static int gc_collecting = 0;
#else
static atomic_int gc_collecting = 0;
#endif

// Mark worklist (raw malloc, not GC-tracked)
static void **gc_worklist = NULL;
static size_t gc_worklist_count = 0;
static size_t gc_worklist_cap = 0;

// Interval tables (rebuilt each collection; buffers kept across cycles,
// grow-only, to avoid per-collection malloc/free churn — see end of
// __pluto_gc_collect).
static GCInterval *gc_intervals = NULL;
static size_t gc_interval_count = 0;
static size_t gc_interval_cap = 0;
static GCDataInterval *gc_data_intervals = NULL;
static size_t gc_data_interval_count = 0;
static size_t gc_data_interval_cap = 0;

// Coarse heap bounds over every live GC object AND every data buffer,
// recomputed each collection in gc_build_intervals. A candidate word
// outside [gc_heap_min, gc_heap_max) cannot point into the heap, so the
// pointer-lookup binary searches reject it with a single compare instead
// of an O(log n) search that finds nothing. Most scanned words are small
// integers or unrelated addresses, so this is the common case. Sound:
// every real pointer into a GC object or data buffer lies within these
// bounds by construction, so the fast path never rejects a live pointer.
static void *gc_heap_min = NULL;
static void *gc_heap_max = NULL;

// Exact-start hash set over every live GC object's user pointer, maintained
// incrementally: gc_alloc inserts, the sweep removes dead entries (or rebuilds
// from the survivors when most of the heap died). Two users:
//  - the collector's gc_find_object, which answers pointers to an object's
//    START in O(1) and only falls back to the interval binary search for
//    misses (interior pointers, in-bounds non-pointers). Every hit is an
//    object the search would also have returned, so the collector stays
//    exactly as conservative as before.
//  - __pluto_gc_find_object, the runtime's pointer -> object query behind
//    deep copy and structural equality, which used to walk the whole heap.
// Open addressing, linear probing with backward-shift deletion, power-of-two
// capacity kept >= 2x the live count; NULL marks an empty slot. Production
// mode: read and written only under gc_mutex.
static void **gc_live_tab = NULL;
static size_t gc_live_cap = 0;
static size_t gc_live_count = 0;

static inline size_t gc_ptr_hash(void *p) {
    // malloc results are 16-byte aligned, so drop the always-zero low bits
    // before mixing (fmix64 finalizer).
    uint64_t x = (uint64_t)(uintptr_t)p >> 4;
    x ^= x >> 33;
    x *= 0xff51afd7ed558ccdULL;
    x ^= x >> 33;
    return (size_t)x;
}

static void gc_live_reset(size_t cap) {
    if (cap != gc_live_cap) {
        free(gc_live_tab);
        gc_live_tab = (void **)calloc(cap, sizeof(void *));
        if (!gc_live_tab) {
            fprintf(stderr, "pluto: out of memory (GC object table)\n");
            exit(1);
        }
        gc_live_cap = cap;
    } else {
        memset(gc_live_tab, 0, cap * sizeof(void *));
    }
    gc_live_count = 0;
}

static void gc_live_rehash(size_t new_cap) {
    void **old = gc_live_tab;
    size_t old_cap = gc_live_cap;
    void **t = (void **)calloc(new_cap, sizeof(void *));
    if (!t) {
        fprintf(stderr, "pluto: out of memory (GC object table)\n");
        exit(1);
    }
    size_t mask = new_cap - 1;
    for (size_t k = 0; k < old_cap; k++) {
        void *e = old[k];
        if (!e) continue;
        size_t i = gc_ptr_hash(e) & mask;
        while (t[i]) i = (i + 1) & mask;
        t[i] = e;
    }
    free(old);
    gc_live_tab = t;
    gc_live_cap = new_cap;
}

static inline void gc_live_insert(void *user) {
    if ((gc_live_count + 1) * 2 > gc_live_cap) {
        gc_live_rehash(gc_live_cap ? gc_live_cap * 2 : 1024);
    }
    size_t mask = gc_live_cap - 1;
    size_t i = gc_ptr_hash(user) & mask;
    while (gc_live_tab[i]) i = (i + 1) & mask;
    gc_live_tab[i] = user;
    gc_live_count++;
}

// Backward-shift deletion: after emptying slot i, walk the probe run and pull
// back any entry whose home slot is not cyclically in (i, j], so every
// remaining entry stays reachable from its home without tombstones.
static void gc_live_delete(void *user) {
    if (!gc_live_cap) return;
    size_t mask = gc_live_cap - 1;
    size_t i = gc_ptr_hash(user) & mask;
    while (gc_live_tab[i] != user) {
        if (!gc_live_tab[i]) return;  // not present
        i = (i + 1) & mask;
    }
    size_t j = i;
    for (;;) {
        j = (j + 1) & mask;
        void *e = gc_live_tab[j];
        if (!e) break;
        size_t k = gc_ptr_hash(e) & mask;
        int stays = (i <= j) ? (i < k && k <= j) : (i < k || k <= j);
        if (stays) continue;
        gc_live_tab[i] = e;
        i = j;
    }
    gc_live_tab[i] = NULL;
    gc_live_count--;
}

// New objects are not hashed at allocation time: a random write into a
// multi-megabyte table on every allocation would cost a cache miss on the
// hottest path in the runtime. gc_alloc only PREPENDS to gc_head, so the
// objects not yet in the table are exactly the list prefix in front of
// gc_hashed_upto (the head as of the last flush or sweep). The table is
// brought up to date by walking that prefix only when it is about to be
// consulted — at the start of each collection and by __pluto_gc_find_object.
// Nothing between sweeps frees list nodes, so gc_hashed_upto stays valid.
static GCHeader *gc_hashed_upto = NULL;

// Scratch for a flush: the prefix is gathered first so the inserts can
// prefetch their slots a fixed distance ahead (the table is usually
// cache-cold here; keeping many misses in flight hides most of the latency).
static void **gc_flush_buf = NULL;
static size_t gc_flush_cap = 0;
#define GC_FLUSH_PREFETCH 16

static inline void gc_flush_buf_push(size_t n, void *user) {
    if (n == gc_flush_cap) {
        size_t cap = gc_flush_cap ? gc_flush_cap * 2 : 4096;
        void **grown = (void **)realloc(gc_flush_buf, cap * sizeof(void *));
        if (!grown) {
            fprintf(stderr, "pluto: out of memory (GC object table)\n");
            exit(1);
        }
        gc_flush_buf = grown;
        gc_flush_cap = cap;
    }
    gc_flush_buf[n] = user;
}

// Insert the n gathered pointers in gc_flush_buf into the live table.
static void gc_live_insert_gathered(size_t n) {
    if (n == 0) return;
    // Grow once up front so slot positions stay valid while prefetching.
    while ((gc_live_count + n) * 2 > gc_live_cap) {
        gc_live_rehash(gc_live_cap ? gc_live_cap * 2 : 1024);
    }
    size_t mask = gc_live_cap - 1;
    for (size_t k = 0; k < n; k++) {
        if (k + GC_FLUSH_PREFETCH < n) {
            __builtin_prefetch(
                &gc_live_tab[gc_ptr_hash(gc_flush_buf[k + GC_FLUSH_PREFETCH]) & mask], 1);
        }
        gc_live_insert(gc_flush_buf[k]);
    }
}

static void gc_live_flush(void) {
    size_t n = 0;
    for (GCHeader *h = gc_head; h && h != gc_hashed_upto; h = h->next) {
        gc_flush_buf_push(n++, (char *)h + sizeof(GCHeader));
    }
    gc_hashed_upto = gc_head;
    gc_live_insert_gathered(n);
}

static inline GCHeader *gc_find_start(void *candidate) {
    if (!gc_live_cap) return NULL;
    size_t mask = gc_live_cap - 1;
    size_t i = gc_ptr_hash(candidate) & mask;
    for (;;) {
        void *e = gc_live_tab[i];
        if (!e) return NULL;
        if (e == candidate) return (GCHeader *)((char *)e - sizeof(GCHeader));
        i = (i + 1) & mask;
    }
}

// Objects marked in the current cycle (sweep uses it to choose between
// deleting dead table entries and rebuilding from survivors).
static size_t gc_marked_count = 0;

// Thread-local storage definitions (referenced in header, defined here)
__thread void *__pluto_current_error = NULL;
__thread void *__pluto_current_error_type = NULL;
__thread long *__pluto_current_task = NULL;

// Fiber stack registry for GC scanning (test mode only).
// The fiber scheduler populates this so __pluto_gc_collect can scan fiber stacks.
#ifdef PLUTO_TEST_MODE
#define GC_MAX_FIBER_STACKS 256
typedef struct {
    char *base;        // malloc'd stack base
    size_t size;       // stack allocation size
    int active;        // 1 if fiber is not completed
} GCFiberStack;
static struct {
    GCFiberStack stacks[GC_MAX_FIBER_STACKS];
    int count;
    int current_fiber;  // index of currently running fiber (-1 if none)
    int enabled;        // 1 when scheduler is active
} gc_fiber_stacks = { .current_fiber = -1, .enabled = 0 };

// Lowest live main-thread stack address while a fiber runs: scheduler_run
// records its own frame before swapping into a fiber, so a fiber-triggered
// collection can scan the frozen main stack [floor, gc_stack_bottom)
// without guessing the extent from inside the fiber.
static void *gc_main_stack_floor = NULL;

void __pluto_gc_set_main_stack_floor(void *floor) {
    gc_main_stack_floor = floor;
}

// The Scheduler allocation is a GC root region: it is the ONLY holder of
// some references — fibers[i].task for un-awaited tasks, closure_ptr of a
// spawned-but-not-yet-started fiber, blocked_value of a parked sender,
// saved TLS, and each suspended fiber's callee-saved registers inside its
// ucontext_t. Scanned conservatively like a stack. (Before this existed,
// those refs survived only because the broken fiber-triggered stack scan
// happened to sweep the malloc region containing the Scheduler.)
static void *gc_scheduler_region = NULL;
static size_t gc_scheduler_region_size = 0;

void __pluto_gc_set_scheduler_region(void *base, size_t size) {
    gc_scheduler_region = base;
    gc_scheduler_region_size = size;
}

// Fiber stack API for scheduler (test mode only)
//
// The registry is RUN-SCOPED: test_run_single frees every fiber stack at the
// end of a schedule run, so entries must not survive into the next run. The
// scheduler resets before registering fiber 0. Without the reset the registry
// fills with dangling pointers across schedule re-runs (a collection then
// scans freed stacks), registration silently stops at the cap, and
// mark_fiber_complete's per-run fiber id misindexes the cumulative array.
void __pluto_gc_reset_fiber_stacks(void) {
    gc_fiber_stacks.count = 0;
    gc_fiber_stacks.current_fiber = -1;
    gc_fiber_stacks.enabled = 0;
}

void __pluto_gc_register_fiber_stack(char *base, size_t size) {
    if (gc_fiber_stacks.count < GC_MAX_FIBER_STACKS) {
        gc_fiber_stacks.stacks[gc_fiber_stacks.count].base = base;
        gc_fiber_stacks.stacks[gc_fiber_stacks.count].size = size;
        gc_fiber_stacks.stacks[gc_fiber_stacks.count].active = 1;
        gc_fiber_stacks.count++;
    }
}

void __pluto_gc_mark_fiber_complete(int fiber_id) {
    if (fiber_id >= 0 && fiber_id < gc_fiber_stacks.count) {
        gc_fiber_stacks.stacks[fiber_id].active = 0;
    }
}

void __pluto_gc_set_current_fiber(int fiber_id) {
    gc_fiber_stacks.current_fiber = fiber_id;
}

void __pluto_gc_enable_fiber_scanning(void) {
    gc_fiber_stacks.enabled = 1;
}

void __pluto_gc_disable_fiber_scanning(void) {
    gc_fiber_stacks.enabled = 0;
}
#endif

// GC thread safety (production mode only)
#ifndef PLUTO_TEST_MODE
static pthread_mutex_t gc_mutex = PTHREAD_MUTEX_INITIALIZER;
static atomic_int __pluto_active_tasks = 0;

// Thread registry for stop-the-world GC (dynamic, slots reused).
// Each spawned thread registers itself so the GC can coordinate safepoints
// and scan its stack.
//
// Slots are individually heap-allocated and NEVER freed or moved: a parking
// thread writes its own park record (stack_cur/park_regs, below) through the
// thread-local gc_my_slot pointer WITHOUT holding gc_mutex, so slot addresses
// must stay stable. The pointer table and slot (de)activation are mutated
// only under gc_mutex; the collector holds gc_mutex for the whole collection,
// so the table is stable while scanning.
typedef struct {
    pthread_t thread;
    void *stack_lo;
    void *stack_hi;
    // Park record, written by the owning thread in gc_record_park() just
    // before it counts itself parked (safepoint stop or safe-region entry):
    // stack_cur is (approximately) the thread's stack pointer at that moment,
    // park_regs a setjmp snapshot of its callee-saved registers. The collector
    // scans [stack_cur, stack_hi) plus park_regs instead of the full stack
    // reservation — see the comment at the 3c scan for the soundness argument.
    // NULL stack_cur means "never parked"; the collector then falls back to
    // scanning the full reservation.
    void *stack_cur;
    jmp_buf park_regs;
    int active;
} GCThreadStack;
static GCThreadStack **gc_thread_stacks = NULL;
static int gc_thread_stack_count = 0;   // high-water slot count
static int gc_thread_stack_cap = 0;
static int gc_active_thread_count = 0;  // currently active slots (under gc_mutex)
// This thread's registry slot (NULL when unregistered). Set/cleared under
// gc_mutex at (de)registration; read lock-free by the owning thread only.
static __thread GCThreadStack *gc_my_slot = NULL;

// Pending-task roots: a spawned task handle is only reachable from the new
// thread's stack, and that stack isn't registered until the trampoline runs.
// The spawner parks the handle here before pthread_create; the trampoline
// removes it after registering its stack. Scanned as explicit GC roots.
static void **gc_pending_roots = NULL;
static int gc_pending_root_count = 0;
static int gc_pending_root_cap = 0;

// Stop-the-world state. The collector sets gc_safepoint_requested and waits
// until every other registered thread is parked: either stopped at a
// safepoint (gc_stw_stopped) or blocked in a safe region (gc_safe_count) —
// a region that cannot touch the GC heap (cond waits, blocking syscalls,
// waiting for gc_mutex itself). After collecting, the collector clears the
// request and waits for every safepoint-stopped thread to acknowledge resume
// (gc_stw_resumed) before the counters can be reused — a thread still inside
// the old cycle's spin can therefore never be confused with the next cycle.
static atomic_int gc_safepoint_requested = 0;
static atomic_int gc_stw_stopped = 0;
static atomic_int gc_stw_resumed = 0;
static atomic_int gc_safe_count = 0;
// Set while this thread has an active registry slot. Safe-region accounting
// must be invisible for unregistered threads: the collector's closing
// condition is `stopped + safe >= count(registered)`, and an UNREGISTERED
// thread parked in a safe region (e.g. blocked in gc_heap_lock during its
// own registration) would inflate `safe` and stand in for a still-running
// registered thread — releasing the collector to scan a heap that thread is
// mutating. An unregistered thread is not in `count`, so not counting it is
// exactly right: it only ever blocks on gc_mutex and cannot touch the heap.
static __thread int gc_thread_registered = 0;

// Record this thread's park site in its registry slot: a setjmp snapshot of
// the callee-saved registers and the current stack pointer. Called just
// before the thread counts itself parked (safepoint stop or safe-region
// entry). Ordering: these plain writes happen-before the seq_cst increment of
// gc_stw_stopped / gc_safe_count that follows at every call site, and the
// collector only reads the slot after observing that increment in its
// stop-the-world closing condition — so the collector never sees a torn
// record for a thread it is entitled to scan.
//
// noinline so the anchor reliably sits below every caller frame; the exact
// depth only needs to be at-or-below the deepest frame that can hold a GC
// reference (deeper is merely a slight over-scan).
#if defined(__GNUC__) || defined(__clang__)
__attribute__((noinline))
#endif
static void gc_record_park(void) {
    GCThreadStack *slot = gc_my_slot;
    if (!slot) return;
    setjmp(slot->park_regs);
    volatile char anchor = 0;
    (void)anchor;
    slot->stack_cur = (void *)&anchor;
}

// Safepoint check - called by threads at regular intervals (loop back-edges,
// runtime waits). If GC has requested a safepoint, park here until it's done.
void __pluto_safepoint(void) {
    if (atomic_load(&gc_safepoint_requested) == 0) {
        return;  // Fast path - no GC pending
    }

    // Record the park site: flushes callee-saved registers into our registry
    // slot and publishes the live stack extent for the collector's scan.
    // (Caller-saved registers holding GC refs were spilled to our frames —
    // above the recorded SP — by the compiler around this very call.)
    gc_record_park();

    atomic_fetch_add(&gc_stw_stopped, 1);
    while (atomic_load(&gc_safepoint_requested)) {
        usleep(100);
    }
    // Acknowledge resume: the collector waits for this before starting a new
    // cycle, so the stop/resume counters can't be reset out from under us.
    atomic_fetch_add(&gc_stw_resumed, 1);
}

// A safe region brackets code that blocks without touching the GC heap.
// While inside, the thread counts as stopped for stop-the-world purposes.
//
// The park record taken here stays valid for the whole stint, including the
// park in __pluto_gc_leave_safe_region on the way out: between enter and a
// successful leave the thread runs only safe-region code, which by contract
// never touches the GC heap — so it cannot acquire a GC reference it didn't
// already hold at entry (entry-time references are covered by park_regs plus
// the stack above stack_cur), and deeper frames (pthread/syscall internals)
// only ever spill those same entry-time register values.
void __pluto_gc_enter_safe_region(void) {
    if (!gc_thread_registered) return;
    gc_record_park();
    atomic_fetch_add(&gc_safe_count, 1);
}

void __pluto_gc_leave_safe_region(void) {
    if (!gc_thread_registered) return;
    for (;;) {
        if (atomic_load(&gc_safepoint_requested) == 0) {
            atomic_fetch_sub(&gc_safe_count, 1);
            // A collection may have started between the check and the
            // decrement; re-check before returning to heap-touching code.
            if (atomic_load(&gc_safepoint_requested) == 0) {
                return;
            }
            atomic_fetch_add(&gc_safe_count, 1);  // undo; park below
        }
        while (atomic_load(&gc_safepoint_requested)) {
            usleep(100);
        }
    }
}

// Safe-region entry WITHOUT the park-site snapshot. The snapshot (setjmp +
// SP) costs real time, and gc_heap_lock sits on the allocation fast path, so
// taking it per-allocation would tax every program. Instead, clear any stale
// park record: if the collector catches this thread mid-wait it falls back
// to the full-reservation scan, which is what the pre-high-water-mark
// collector always did and remains sound (callee-saved registers holding GC
// refs are spilled into pthread_mutex_lock's own frames, which the full
// range covers). This only happens when a collection overlaps the short
// mutex wait, so the fallback's extra cost is rare.
static void gc_enter_safe_region_nosnapshot(void) {
    if (!gc_thread_registered) return;
    if (gc_my_slot) gc_my_slot->stack_cur = NULL;
    atomic_fetch_add(&gc_safe_count, 1);
}

// Acquire gc_mutex, counting the (possibly long) wait as a safe region: the
// collector holds gc_mutex for the entire collection, and a thread blocked
// here must not stall it. Holding gc_mutex implies no collection is running,
// so the leave on the way out never parks.
static void gc_heap_lock(void) {
    gc_enter_safe_region_nosnapshot();
    pthread_mutex_lock(&gc_mutex);
    __pluto_gc_leave_safe_region();
}

// Thread registration API for spawned tasks
void __pluto_gc_register_thread_stack(void *stack_lo, void *stack_hi) {
    gc_heap_lock();
    GCThreadStack *slot = NULL;
    for (int i = 0; i < gc_thread_stack_count; i++) {
        if (!gc_thread_stacks[i]->active) { slot = gc_thread_stacks[i]; break; }
    }
    if (!slot) {
        if (gc_thread_stack_count == gc_thread_stack_cap) {
            int new_cap = gc_thread_stack_cap ? gc_thread_stack_cap * 2 : 64;
            GCThreadStack **grown =
                (GCThreadStack **)realloc(gc_thread_stacks, new_cap * sizeof(GCThreadStack *));
            if (!grown) {
                pthread_mutex_unlock(&gc_mutex);
                fprintf(stderr, "pluto: out of memory registering thread\n");
                exit(1);
            }
            gc_thread_stacks = grown;
            gc_thread_stack_cap = new_cap;
        }
        slot = (GCThreadStack *)calloc(1, sizeof(GCThreadStack));
        if (!slot) {
            pthread_mutex_unlock(&gc_mutex);
            fprintf(stderr, "pluto: out of memory registering thread\n");
            exit(1);
        }
        gc_thread_stacks[gc_thread_stack_count++] = slot;
    }
    slot->thread = pthread_self();
    slot->stack_lo = stack_lo;
    slot->stack_hi = stack_hi;
    slot->stack_cur = NULL;   // no park record yet: scan full range if needed
    slot->active = 1;
    gc_active_thread_count++;
    gc_my_slot = slot;
    // Flag and slot flip together under gc_mutex: the collector (which also
    // holds gc_mutex to count) can never see one without the other
    gc_thread_registered = 1;
    pthread_mutex_unlock(&gc_mutex);
}

void __pluto_gc_deregister_thread_stack(void) {
    gc_heap_lock();
    if (gc_my_slot) {
        gc_my_slot->active = 0;
        gc_active_thread_count--;
        gc_my_slot = NULL;
    }
    gc_thread_registered = 0;
    pthread_mutex_unlock(&gc_mutex);
}

// Pending-task root API (used by threading.c around pthread_create)
void __pluto_gc_add_pending_root(void *p) {
    gc_heap_lock();
    if (gc_pending_root_count == gc_pending_root_cap) {
        int new_cap = gc_pending_root_cap ? gc_pending_root_cap * 2 : 16;
        void **grown = (void **)realloc(gc_pending_roots, new_cap * sizeof(void *));
        if (!grown) {
            pthread_mutex_unlock(&gc_mutex);
            fprintf(stderr, "pluto: out of memory tracking task root\n");
            exit(1);
        }
        gc_pending_roots = grown;
        gc_pending_root_cap = new_cap;
    }
    gc_pending_roots[gc_pending_root_count++] = p;
    pthread_mutex_unlock(&gc_mutex);
}

void __pluto_gc_remove_pending_root(void *p) {
    gc_heap_lock();
    for (int i = 0; i < gc_pending_root_count; i++) {
        if (gc_pending_roots[i] == p) {
            gc_pending_roots[i] = gc_pending_roots[--gc_pending_root_count];
            break;
        }
    }
    pthread_mutex_unlock(&gc_mutex);
}

// Fork support. The forking thread holds gc_mutex across fork() so the child
// inherits consistent heap metadata and an unheld (held-by-us) allocator lock.
// In the child, only the forking thread survives: every other registry entry
// is a ghost whose stack no longer runs, and any of them counted in a
// stop-the-world wait would hang the child's first collection — so the child
// resets all GC coordination state to a single-threaded baseline.
void __pluto_gc_prepare_fork(void) {
    gc_heap_lock();
}

void __pluto_gc_after_fork(int is_child) {
    if (is_child) {
        // Deactivate every registry slot except the surviving (current) thread.
        pthread_t self = pthread_self();
        gc_active_thread_count = 0;
        for (int i = 0; i < gc_thread_stack_count; i++) {
            if (gc_thread_stacks[i]->active
                && !pthread_equal(gc_thread_stacks[i]->thread, self)) {
                gc_thread_stacks[i]->active = 0;
            }
            if (gc_thread_stacks[i]->active) gc_active_thread_count++;
        }
        // Ghost threads can no longer leave safe regions or ack a resume.
        atomic_store(&gc_safe_count, 0);
        atomic_store(&gc_stw_stopped, 0);
        atomic_store(&gc_stw_resumed, 0);
        atomic_store(&gc_safepoint_requested, 0);
        atomic_store(&__pluto_active_tasks, 0);
        // Parent's in-flight spawns will never register here; drop their roots.
        gc_pending_root_count = 0;
    }
    pthread_mutex_unlock(&gc_mutex);
}

int __pluto_gc_active_tasks(void) {
    return atomic_load(&__pluto_active_tasks);
}

void __pluto_gc_task_start(void) {
    atomic_fetch_add(&__pluto_active_tasks, 1);
}

void __pluto_gc_task_end(void) {
    atomic_fetch_sub(&__pluto_active_tasks, 1);
}
#else
// No-op safepoint for test mode (single-threaded, no GC coordination needed)
void __pluto_safepoint(void) {
    // Test mode: no-op
}
void __pluto_gc_enter_safe_region(void) {}
void __pluto_gc_leave_safe_region(void) {}
void __pluto_gc_add_pending_root(void *p) { (void)p; }
void __pluto_gc_remove_pending_root(void *p) { (void)p; }
void __pluto_gc_prepare_fork(void) {}
void __pluto_gc_after_fork(int is_child) { (void)is_child; }
#endif

// ── Global roots ──────────────────────────────────────────────────────────────
//
// Addresses of module globals that hold GC references — today the
// __pluto_singleton_* slots written by DI startup wiring (issue #434). A heap
// value reachable ONLY through such a global (e.g. a singleton consumed
// exclusively by scope blocks) has no stack or register presence after
// startup, so without this registry the mark phase would treat it as garbage.
// The registry stores slot ADDRESSES: the collector re-reads each slot every
// cycle, so later writes through the global are picked up naturally.
//
// Registration is done by generated startup code on the main thread, before
// any task threads exist, but it interleaves with allocation (each singleton
// is allocated, stored to its global, then registered), so production mode
// takes the heap lock like the pending-root API does. The table is raw
// malloc, never GC memory.
static void **gc_global_roots = NULL;
static int gc_global_root_count = 0;
static int gc_global_root_cap = 0;

void __pluto_gc_register_global_root(void *slot) {
#ifndef PLUTO_TEST_MODE
    gc_heap_lock();
#endif
    if (gc_global_root_count == gc_global_root_cap) {
        int new_cap = gc_global_root_cap ? gc_global_root_cap * 2 : 16;
        void **grown = (void **)realloc(gc_global_roots, new_cap * sizeof(void *));
        if (!grown) {
#ifndef PLUTO_TEST_MODE
            pthread_mutex_unlock(&gc_mutex);
#endif
            fprintf(stderr, "pluto: out of memory registering global root\n");
            exit(1);
        }
        gc_global_roots = grown;
        gc_global_root_cap = new_cap;
    }
    gc_global_roots[gc_global_root_count++] = slot;
#ifndef PLUTO_TEST_MODE
    pthread_mutex_unlock(&gc_mutex);
#endif
}

// Get GC header from user pointer
static inline GCHeader *gc_get_header(void *user_ptr) {
    return (GCHeader *)((char *)user_ptr - sizeof(GCHeader));
}

// ── Allocation ────────────────────────────────────────────────────────────────

#ifdef PLUTO_TEST_MODE
void *gc_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count) {
    // Test mode: single-threaded, no mutex needed
    if (gc_stack_bottom && !gc_collecting
        && gc_bytes_allocated + user_size + sizeof(GCHeader) > gc_threshold) {
        __pluto_gc_collect();
    }
    size_t total = sizeof(GCHeader) + user_size;
    GCHeader *h = (GCHeader *)calloc(1, total);
    if (!h) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    h->next = gc_head;
    gc_head = h;
    h->size = (uint32_t)user_size;
    h->type_tag = type_tag;
    h->field_count = field_count;
    h->mark = 0;
    gc_bytes_allocated += total;
    return (char *)h + sizeof(GCHeader);
}
#else
// Stop all other registered threads. Called with gc_mutex held.
// Sets the safepoint request and waits until every other active registered
// thread is either stopped at a safepoint or parked in a safe region.
// Returns the number of safepoint-stopped threads, which the caller must pass
// to gc_stw_resume_threads() after collecting.
static int gc_stw_stop_threads(void) {
    atomic_store(&gc_stw_stopped, 0);
    atomic_store(&gc_stw_resumed, 0);

    // Count active threads (excluding self)
    pthread_t self = pthread_self();
    int count = 0;
    for (int i = 0; i < gc_thread_stack_count; i++) {
        if (!gc_thread_stacks[i]->active) continue;
        if (pthread_equal(gc_thread_stacks[i]->thread, self)) continue;
        count++;
    }
    if (count == 0) return 0;

    atomic_store(&gc_safepoint_requested, 1);

    // No timeout: every registered thread either polls safepoints (loop
    // back-edges, runtime waits) or parks in a safe region around anything
    // that blocks, so this converges. Proceeding early would let an unpaused
    // thread mutate the heap mid-collection (use-after-free).
    for (;;) {
        int stopped = atomic_load(&gc_stw_stopped);
        int safe = atomic_load(&gc_safe_count);
        if (stopped + safe >= count) break;
        usleep(100);
    }
    return atomic_load(&gc_stw_stopped);
}

static void gc_stw_resume_threads(int stopped_count) {
    atomic_store(&gc_safepoint_requested, 0);
    // Wait for every safepoint-stopped thread to leave its spin before the
    // counters can be reused by a later cycle.
    while (atomic_load(&gc_stw_resumed) < stopped_count) {
        usleep(100);
    }
}

void *gc_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count) {
    // The wait for gc_mutex counts as a safe region: the collector holds it
    // for the whole collection, and a thread parked here must count as
    // stopped or stop-the-world would deadlock.
    gc_heap_lock();
    if (gc_stack_bottom
        && gc_bytes_allocated + user_size + sizeof(GCHeader) > gc_threshold) {
        // Initiation is serialized by gc_mutex: whoever holds it and sees the
        // threshold exceeded collects. A thread that was parked waiting on
        // gc_mutex during a collection re-checks the (now raised) threshold
        // and usually just allocates.
        int stopped = gc_stw_stop_threads();
        __pluto_gc_collect();
        gc_stw_resume_threads(stopped);
    }
    size_t total = sizeof(GCHeader) + user_size;
    GCHeader *h = (GCHeader *)calloc(1, total);
    if (!h) { pthread_mutex_unlock(&gc_mutex); fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    h->next = gc_head;
    gc_head = h;
    h->size = (uint32_t)user_size;
    h->type_tag = type_tag;
    h->field_count = field_count;
    h->mark = 0;
    gc_bytes_allocated += total;
    pthread_mutex_unlock(&gc_mutex);
    return (char *)h + sizeof(GCHeader);
}
#endif

// Public allocation API
void *__pluto_alloc(long size) {
    if (size == 0) size = 8;
    uint16_t field_count = (uint16_t)(size / 8);
    return gc_alloc((size_t)size, GC_TAG_OBJECT, field_count);
}

/* Entities (object declarations) get their own tag so the runtime can honor
 * identity semantics structurally: deep_copy shares them, deep_eq compares
 * them by pointer. Mark-phase tracing uses the conservative default case.
 *
 * Each entity carries one hidden trailing slot holding its per-instance
 * method rwlock (see __pluto_entity_rdlock/wrlock in threading.c). The slot
 * sits past field_count, so the mark phase never scans it (it is a malloc'd
 * pointer, not a GC reference), and deep_copy/deep_eq/marshal never see it
 * (entities are shared by identity, never traversed). */
void *__pluto_alloc_entity(long size) {
    if (size == 0) size = 8;
    uint16_t field_count = (uint16_t)(size / 8);
    long *ptr = (long *)gc_alloc((size_t)size + 8, GC_TAG_ENTITY, field_count);
    // Both modes: test mode now carries a real fiber-aware lock too
    // (rfc-test-harness phase 4 — lock sites are preemption points).
    ptr[size / 8] = __pluto_rwlock_init();
    return ptr;
}

// ── Interval table for pointer lookup ─────────────────────────────────────────

static int gc_interval_cmp(const void *a, const void *b) {
    const GCInterval *ia = (const GCInterval *)a;
    const GCInterval *ib = (const GCInterval *)b;
    if (ia->start < ib->start) return -1;
    if (ia->start > ib->start) return 1;
    return 0;
}

static int gc_data_interval_cmp(const void *a, const void *b) {
    const GCDataInterval *ia = (const GCDataInterval *)a;
    const GCDataInterval *ib = (const GCDataInterval *)b;
    if (ia->start < ib->start) return -1;
    if (ia->start > ib->start) return 1;
    return 0;
}

// ── Interval sorting ───────────────────────────────────────────────────────
//
// Both interval kinds share one layout, { start, end, owner }, and are sorted
// by start for the binary-search fallback lookups. qsort through a comparator
// function pointer dominated collection time on large heaps (it ran over
// every object, every cycle), so sort with an LSD radix sort on the address
// offset instead: key = (start - min) >> 4 (allocations are 16-byte aligned),
// 11 bits per pass, so a few O(n) passes and no comparator calls.
//
// Correctness never depends on the radix sort: the result is verified with a
// linear sortedness check and, if that ever fails (or the scratch buffer can't
// be allocated), the table is re-sorted with qsort. A radix-sort bug can cost
// time, not a missed mark.
typedef struct { void *start; void *end; void *owner; } GCSortInterval;
_Static_assert(sizeof(GCInterval) == sizeof(GCSortInterval), "interval layout");
_Static_assert(sizeof(GCDataInterval) == sizeof(GCSortInterval), "data interval layout");

#define GC_RADIX_BITS 11
#define GC_RADIX_BUCKETS (1u << GC_RADIX_BITS)
#define GC_RADIX_MIN_N 256   // below this, qsort is already cheap

static GCSortInterval *gc_sort_tmp = NULL;
static size_t gc_sort_tmp_cap = 0;

static void gc_sort_intervals(GCSortInterval *a, size_t n,
                              int (*cmp)(const void *, const void *)) {
    if (n < 2) return;
    if (n >= GC_RADIX_MIN_N) {
        if (n > gc_sort_tmp_cap) {
            GCSortInterval *grown =
                (GCSortInterval *)realloc(gc_sort_tmp, n * sizeof(GCSortInterval));
            if (grown) {
                gc_sort_tmp = grown;
                gc_sort_tmp_cap = n;
            }
        }
        if (n <= gc_sort_tmp_cap) {
            uintptr_t lo = UINTPTR_MAX, hi = 0;
            for (size_t i = 0; i < n; i++) {
                uintptr_t s = (uintptr_t)a[i].start;
                if (s < lo) lo = s;
                if (s > hi) hi = s;
            }
            uintptr_t span = (hi - lo) >> 4;
            int bits = 0;
            while (bits < 64 && (span >> bits) != 0) bits++;

            GCSortInterval *src = a, *dst = gc_sort_tmp;
            size_t cnt[GC_RADIX_BUCKETS];
            for (int shift = 0; shift < bits; shift += GC_RADIX_BITS) {
                memset(cnt, 0, sizeof cnt);
                for (size_t i = 0; i < n; i++) {
                    cnt[((((uintptr_t)src[i].start - lo) >> 4) >> shift)
                        & (GC_RADIX_BUCKETS - 1)]++;
                }
                size_t sum = 0;
                for (size_t b = 0; b < GC_RADIX_BUCKETS; b++) {
                    size_t c = cnt[b];
                    cnt[b] = sum;
                    sum += c;
                }
                for (size_t i = 0; i < n; i++) {
                    size_t k = ((((uintptr_t)src[i].start - lo) >> 4) >> shift)
                               & (GC_RADIX_BUCKETS - 1);
                    dst[cnt[k]++] = src[i];
                }
                GCSortInterval *t = src;
                src = dst;
                dst = t;
            }
            if (src != a) memcpy(a, src, n * sizeof(GCSortInterval));

            int sorted = 1;
            for (size_t i = 1; i < n; i++) {
                if (a[i - 1].start > a[i].start) { sorted = 0; break; }
            }
            if (sorted) return;
        }
    }
    qsort(a, n, sizeof(GCSortInterval), cmp);
}

static void gc_build_intervals(void) {
    // Count objects. The same walk gathers the not-yet-hashed list prefix
    // (objects allocated since the last flush/sweep), so every object is in
    // the live table before marking consults it — without a second walk.
    size_t count = 0;
    size_t data_buf_count = 0;
    size_t fresh = 0;
    int in_fresh_prefix = 1;
    for (GCHeader *h = gc_head; h; h = h->next) {
        if (h == gc_hashed_upto) in_fresh_prefix = 0;
        if (in_fresh_prefix) gc_flush_buf_push(fresh++, (char *)h + sizeof(GCHeader));
        count++;
        if (h->type_tag == GC_TAG_ARRAY) data_buf_count++;
        else if (h->type_tag == GC_TAG_BYTES) data_buf_count++;
        else if (h->type_tag == GC_TAG_MAP) data_buf_count += 3;  // keys, vals, meta
        else if (h->type_tag == GC_TAG_SET) data_buf_count += 2;  // keys, meta
    }
    gc_hashed_upto = gc_head;
    gc_live_insert_gathered(fresh);

    if (count > gc_interval_cap) {
        gc_intervals = (GCInterval *)realloc(gc_intervals, count * sizeof(GCInterval));
        gc_interval_cap = count;
    }
    gc_interval_count = count;
    if (data_buf_count > gc_data_interval_cap) {
        gc_data_intervals =
            (GCDataInterval *)realloc(gc_data_intervals, data_buf_count * sizeof(GCDataInterval));
        gc_data_interval_cap = data_buf_count;
    }
    gc_data_interval_count = 0;


    size_t i = 0;
    for (GCHeader *h = gc_head; h; h = h->next) {
        void *user = (char *)h + sizeof(GCHeader);
        gc_intervals[i].start = user;
        gc_intervals[i].end = (char *)user + h->size;
        gc_intervals[i].header = h;
        i++;

        if (h->type_tag == GC_TAG_ARRAY && h->size >= 24) {
            long *handle = (long *)user;
            long cap = handle[1];
            void *data_ptr = (void *)handle[2];
            if (data_ptr && cap > 0) {
                gc_data_intervals[gc_data_interval_count].start = data_ptr;
                gc_data_intervals[gc_data_interval_count].end = (char *)data_ptr + cap * 8;
                gc_data_intervals[gc_data_interval_count].array_handle = user;
                gc_data_interval_count++;
            }
        }
        // Bytes handle: [len][cap][data_ptr]
        if (h->type_tag == GC_TAG_BYTES && h->size >= 24) {
            long *handle = (long *)user;
            long cap = handle[1];
            void *data_ptr = (void *)handle[2];
            if (data_ptr && cap > 0) {
                gc_data_intervals[gc_data_interval_count].start = data_ptr;
                gc_data_intervals[gc_data_interval_count].end = (char *)data_ptr + cap * 1;
                gc_data_intervals[gc_data_interval_count].array_handle = user;
                gc_data_interval_count++;
            }
        }
        // Map handle: [count][cap][keys_ptr][vals_ptr][meta_ptr]
        if (h->type_tag == GC_TAG_MAP && h->size >= 40) {
            long *mh = (long *)user;
            long cap = mh[1];
            if (cap > 0) {
                void *keys = (void *)mh[2]; void *vals = (void *)mh[3]; void *meta = (void *)mh[4];
                if (keys) { gc_data_intervals[gc_data_interval_count].start = keys; gc_data_intervals[gc_data_interval_count].end = (char *)keys + cap * 8; gc_data_intervals[gc_data_interval_count].array_handle = user; gc_data_interval_count++; }
                if (vals) { gc_data_intervals[gc_data_interval_count].start = vals; gc_data_intervals[gc_data_interval_count].end = (char *)vals + cap * 8; gc_data_intervals[gc_data_interval_count].array_handle = user; gc_data_interval_count++; }
                if (meta) { gc_data_intervals[gc_data_interval_count].start = meta; gc_data_intervals[gc_data_interval_count].end = (char *)meta + cap; gc_data_intervals[gc_data_interval_count].array_handle = user; gc_data_interval_count++; }
            }
        }
        // Set handle: [count][cap][keys_ptr][meta_ptr]
        if (h->type_tag == GC_TAG_SET && h->size >= 32) {
            long *sh = (long *)user;
            long cap = sh[1];
            if (cap > 0) {
                void *keys = (void *)sh[2]; void *meta = (void *)sh[3];
                if (keys) { gc_data_intervals[gc_data_interval_count].start = keys; gc_data_intervals[gc_data_interval_count].end = (char *)keys + cap * 8; gc_data_intervals[gc_data_interval_count].array_handle = user; gc_data_interval_count++; }
                if (meta) { gc_data_intervals[gc_data_interval_count].start = meta; gc_data_intervals[gc_data_interval_count].end = (char *)meta + cap; gc_data_intervals[gc_data_interval_count].array_handle = user; gc_data_interval_count++; }
            }
        }
    }

    gc_sort_intervals((GCSortInterval *)gc_intervals, gc_interval_count, gc_interval_cmp);
    gc_sort_intervals((GCSortInterval *)gc_data_intervals, gc_data_interval_count,
                      gc_data_interval_cmp);

    // Coarse bounds over objects + data buffers for the fast-reject path in
    // gc_find_object / gc_find_array_owner. One sequential O(n) pass.
    if (gc_interval_count == 0 && gc_data_interval_count == 0) {
        gc_heap_min = NULL;
        gc_heap_max = NULL;
    } else {
        void *lo = (void *)~(size_t)0;
        void *hi = NULL;
        for (size_t k = 0; k < gc_interval_count; k++) {
            if (gc_intervals[k].start < lo) lo = gc_intervals[k].start;
            if (gc_intervals[k].end > hi) hi = gc_intervals[k].end;
        }
        for (size_t k = 0; k < gc_data_interval_count; k++) {
            if (gc_data_intervals[k].start < lo) lo = gc_data_intervals[k].start;
            if (gc_data_intervals[k].end > hi) hi = gc_data_intervals[k].end;
        }
        gc_heap_min = lo;
        gc_heap_max = hi;
    }
}

// Binary search: find GC object containing candidate pointer
static GCHeader *gc_find_object(void *candidate) {
    if (gc_interval_count == 0) return NULL;
    if (candidate < gc_heap_min || candidate >= gc_heap_max) return NULL;
    // Fast path: a pointer to an object's start (the common case).
    GCHeader *exact = gc_find_start(candidate);
    if (exact) return exact;
    size_t lo = 0, hi = gc_interval_count;
    while (lo < hi) {
        size_t mid = lo + (hi - lo) / 2;
        if (candidate < gc_intervals[mid].start) {
            hi = mid;
        } else if (candidate >= gc_intervals[mid].end) {
            lo = mid + 1;
        } else {
            return gc_intervals[mid].header;
        }
    }
    return NULL;
}

// Binary search: find array handle owning a data buffer containing candidate
static void *gc_find_array_owner(void *candidate) {
    if (gc_data_interval_count == 0) return NULL;
    if (candidate < gc_heap_min || candidate >= gc_heap_max) return NULL;
    size_t lo = 0, hi = gc_data_interval_count;
    while (lo < hi) {
        size_t mid = lo + (hi - lo) / 2;
        if (candidate < gc_data_intervals[mid].start) {
            hi = mid;
        } else if (candidate >= gc_data_intervals[mid].end) {
            lo = mid + 1;
        } else {
            return gc_data_intervals[mid].array_handle;
        }
    }
    return NULL;
}

// ── Mark phase ────────────────────────────────────────────────────────────────

static void gc_worklist_push(void *ptr) {
    if (gc_worklist_count >= gc_worklist_cap) {
        gc_worklist_cap = gc_worklist_cap ? gc_worklist_cap * 2 : 256;
        gc_worklist = (void **)realloc(gc_worklist, gc_worklist_cap * sizeof(void *));
    }
    gc_worklist[gc_worklist_count++] = ptr;
}

static void gc_mark_object(void *user_ptr) {
    GCHeader *h = gc_get_header(user_ptr);
    if (h->mark) return;
    h->mark = 1;
    gc_marked_count++;
    gc_worklist_push(user_ptr);
}

static void gc_trace_object(void *user_ptr) {
    GCHeader *h = gc_get_header(user_ptr);
    switch (h->type_tag) {
    case GC_TAG_STRING:
    case GC_TAG_BYTES:
        // No child pointers (bytes data is raw u8 values, not GC pointers)
        break;
    case GC_TAG_ARRAY: {
        // Array handle: [len][cap][data_ptr]
        long *handle = (long *)user_ptr;
        long len = handle[0];
        long *data = (long *)handle[2];
        // Scan elements conservatively
        for (long i = 0; i < len; i++) {
            void *candidate = (void *)data[i];
            GCHeader *child = gc_find_object(candidate);
            if (child && !child->mark) {
                void *child_user = (char *)child + sizeof(GCHeader);
                gc_mark_object(child_user);
            }
        }
        break;
    }
    case GC_TAG_TRAIT: {
        // Trait handle: [data_ptr][vtable_ptr]
        long *slots = (long *)user_ptr;
        void *data_ptr = (void *)slots[0];
        GCHeader *child = gc_find_object(data_ptr);
        if (child && !child->mark) {
            void *child_user = (char *)child + sizeof(GCHeader);
            gc_mark_object(child_user);
        }
        break;
    }
    case GC_TAG_MAP: {
        // Map handle: [count][cap][keys_ptr][vals_ptr][meta_ptr]
        long *mh = (long *)user_ptr;
        long count = mh[0]; long cap = mh[1];
        long *keys = (long *)mh[2]; long *vals = (long *)mh[3];
        unsigned char *meta = (unsigned char *)mh[4];
        for (long i = 0; i < cap; i++) {
            if (meta[i] >= 0x80) {
                void *k = (void *)keys[i]; void *v = (void *)vals[i];
                GCHeader *kh = gc_find_object(k);
                if (kh && !kh->mark) gc_mark_object((char *)kh + sizeof(GCHeader));
                GCHeader *vh = gc_find_object(v);
                if (vh && !vh->mark) gc_mark_object((char *)vh + sizeof(GCHeader));
            }
        }
        (void)count;
        break;
    }
    case GC_TAG_SET: {
        // Set handle: [count][cap][keys_ptr][meta_ptr]
        long *sh = (long *)user_ptr;
        long count = sh[0]; long cap = sh[1];
        long *keys = (long *)sh[2];
        unsigned char *meta = (unsigned char *)sh[3];
        for (long i = 0; i < cap; i++) {
            if (meta[i] >= 0x80) {
                void *k = (void *)keys[i];
                GCHeader *kh = gc_find_object(k);
                if (kh && !kh->mark) gc_mark_object((char *)kh + sizeof(GCHeader));
            }
        }
        (void)count;
        break;
    }
    case GC_TAG_STRING_SLICE: {
        // String slice: [backing_ptr][offset][len]; trace backing to keep it alive
        long *slice = (long *)user_ptr;
        void *backing = (void *)slice[0];
        GCHeader *child = gc_find_object(backing);
        if (child && !child->mark) {
            gc_mark_object((char *)child + sizeof(GCHeader));
        }
        break;
    }
    case GC_TAG_CHANNEL: {
        // Channel handle: [sync_ptr][buf_ptr][capacity][count][head][tail][closed]
        long *ch = (long *)user_ptr;
        long *buf = (long *)ch[1];
        long count = ch[3];
        long head = ch[4];
        long capacity = ch[2];
        // Trace live buffer slots (they may hold GC pointers like strings/objects)
        for (long i = 0; i < count; i++) {
            long idx = (head + i) % capacity;
            void *candidate = (void *)buf[idx];
            GCHeader *child = gc_find_object(candidate);
            if (child && !child->mark) {
                gc_mark_object((char *)child + sizeof(GCHeader));
            }
        }
        break;
    }
    case GC_TAG_OBJECT:
    default: {
        // Scan all 8-byte slots conservatively
        long *slots = (long *)user_ptr;
        uint16_t fc = h->field_count;
        for (uint16_t i = 0; i < fc; i++) {
            void *candidate = (void *)slots[i];
            // Check GC objects
            GCHeader *child = gc_find_object(candidate);
            if (child && !child->mark) {
                void *child_user = (char *)child + sizeof(GCHeader);
                gc_mark_object(child_user);
            }
            // Check array data buffers
            void *arr_owner = gc_find_array_owner(candidate);
            if (arr_owner) {
                GCHeader *arr_h = gc_get_header(arr_owner);
                if (!arr_h->mark) {
                    gc_mark_object(arr_owner);
                }
            }
        }
        break;
    }
    }
}

static void gc_mark_candidate(void *candidate) {
    // Check if candidate points into a GC object
    GCHeader *h = gc_find_object(candidate);
    if (h && !h->mark) {
        void *user = (char *)h + sizeof(GCHeader);
        gc_mark_object(user);
    }
    // Check if candidate points into an array data buffer
    void *arr_owner = gc_find_array_owner(candidate);
    if (arr_owner) {
        GCHeader *arr_h = gc_get_header(arr_owner);
        if (!arr_h->mark) {
            gc_mark_object(arr_owner);
        }
    }
}

// ── Garbage Collection ───────────────────────────────────────────────────────

void __pluto_gc_collect(void) {
    gc_collecting = 1;
    if (gc_log_enabled < 0) {
        const char *e = getenv("PLUTO_GC_LOG");
        gc_log_enabled = (e && e[0] == '1') ? 1 : 0;
    }
    struct timespec gc_t0;
    if (gc_log_enabled) clock_gettime(CLOCK_MONOTONIC, &gc_t0);

    // Build interval tables
    gc_build_intervals();
    struct timespec gc_tb;
    if (gc_log_enabled) clock_gettime(CLOCK_MONOTONIC, &gc_tb);

    // Reset worklist
    gc_worklist_count = 0;
    gc_marked_count = 0;

    // 1. Flush registers to stack via setjmp
    jmp_buf regs;
    setjmp(regs);

    // 2. Scan jmp_buf as potential roots
    {
        long *p = (long *)&regs;
        size_t n = sizeof(regs) / (sizeof(long));
        for (size_t i = 0; i < n; i++) {
            gc_mark_candidate((void *)p[i]);
        }
    }

    // 3. Scan the GC-initiating thread's own stack.
    // In production mode, we find this thread's registered stack_hi.
    // In test mode the initiating context may be a FIBER running on a
    // malloc'd 64 KiB heap block, not the main thread stack: scanning
    // [&anchor, gc_stack_bottom) from there walks from the heap across
    // whatever address space separates it from the main stack — unmapped
    // holes → SEGFAULT (it only ever worked when the intervening range
    // happened to be mapped). Scan the fiber's own live stack instead,
    // plus the frozen main stack above the scheduler's switch point
    // (recorded by scheduler_run via __pluto_gc_set_main_stack_floor).
    {
        void *stack_top;
        volatile long anchor = 0;
        (void)anchor;
        stack_top = (void *)&anchor;

#ifndef PLUTO_TEST_MODE
        // Use this thread's registered stack_hi as the scan bound. Without
        // this, a task thread would scan from its stack to gc_stack_bottom
        // (the main thread's stack), crossing unmapped memory → SEGFAULT.
        void *hi = gc_my_slot ? gc_my_slot->stack_hi
                              : gc_stack_bottom;  // fallback: unregistered/main
        void *lo = stack_top;
        // On most platforms stacks grow down, so stack_top < stack_hi.
        // Handle either direction just in case.
        if (lo > hi) { void *tmp = lo; lo = hi; hi = tmp; }
        lo = (void *)(((size_t)lo) & ~7UL);
        for (long *p = (long *)lo; (void *)p < hi; p++) {
            gc_mark_candidate((void *)*p);
        }
#else
        int on_fiber = 0;
        if (gc_fiber_stacks.enabled && gc_fiber_stacks.current_fiber >= 0 &&
            gc_fiber_stacks.current_fiber < gc_fiber_stacks.count) {
            GCFiberStack *cf = &gc_fiber_stacks.stacks[gc_fiber_stacks.current_fiber];
            if (cf->base && (char *)stack_top >= cf->base &&
                (char *)stack_top < cf->base + cf->size) {
                // On the current fiber's stack: its live extent is
                // [stack_top, base + size). The region below SP is dead.
                void *flo = (void *)(((size_t)stack_top) & ~7UL);
                void *fhi = (void *)(cf->base + cf->size);
                for (long *p = (long *)flo; (void *)p < fhi; p++) {
                    gc_mark_candidate((void *)*p);
                }
                // Plus the main thread's frames frozen at the scheduler's
                // swap point: everything above the recorded floor is live
                // (main → __pluto_test_run → test_run_single → scheduler_run);
                // below it sit only swapcontext internals, which hold no
                // GC references.
                if (gc_main_stack_floor && gc_main_stack_floor < gc_stack_bottom) {
                    void *mlo = (void *)(((size_t)gc_main_stack_floor) & ~7UL);
                    for (long *p = (long *)mlo; (void *)p < gc_stack_bottom; p++) {
                        gc_mark_candidate((void *)*p);
                    }
                }
                on_fiber = 1;
            }
        }
        if (!on_fiber) {
            // On the real main thread stack (scheduler context, or
            // sequential mode with no fibers at all).
            void *lo = stack_top < gc_stack_bottom ? stack_top : gc_stack_bottom;
            void *hi = stack_top < gc_stack_bottom ? gc_stack_bottom : stack_top;
            lo = (void *)(((size_t)lo) & ~7UL);
            for (long *p = (long *)lo; (void *)p < hi; p++) {
                gc_mark_candidate((void *)*p);
            }
        }
#endif
    }

#ifdef PLUTO_TEST_MODE
    // 3b. Scan all fiber stacks as additional GC roots.
    // When a fiber triggers GC, the main stack scan above covers the scheduler's
    // stack frames. But other suspended fibers hold live references on their own
    // malloc'd stacks that the GC would miss, potentially collecting live objects.
    if (gc_fiber_stacks.enabled) {
        for (int fi = 0; fi < gc_fiber_stacks.count; fi++) {
            if (!gc_fiber_stacks.stacks[fi].active) continue;
            if (fi == gc_fiber_stacks.current_fiber) continue;  // current fiber's stack was scanned above via anchor
            char *base = gc_fiber_stacks.stacks[fi].base;
            if (!base) continue;
            size_t sz = gc_fiber_stacks.stacks[fi].size;
            // Deliberately NOT high-water-mark scanned (unlike production
            // thread stacks in 3c): a suspended fiber's SP lives inside its
            // ucontext in a platform-specific mcontext layout, and fiber
            // stacks are small (64 KiB), malloc'd and already committed, so
            // the full scan is cheap, page-fault-free, and trivially sound.
            void *flo = (void *)(((size_t)base) & ~7UL);
            void *fhi = (void *)(base + sz);
            for (long *p = (long *)flo; (void *)p < fhi; p++) {
                gc_mark_candidate((void *)*p);
            }
        }
    }

    // 3b'. Scan the Scheduler allocation (sole holder of un-awaited task
    // handles, pending spawn closures, parked send values, and suspended
    // fibers' register state — see __pluto_gc_set_scheduler_region).
    if (gc_scheduler_region && gc_scheduler_region_size > 0) {
        long *sbase = (long *)gc_scheduler_region;
        size_t swords = gc_scheduler_region_size / sizeof(long);
        for (size_t si = 0; si < swords; si++) {
            gc_mark_candidate((void *)sbase[si]);
        }
    }
#endif

#ifndef PLUTO_TEST_MODE
    // 3c. Scan all OTHER registered thread stacks as additional GC roots.
    // The GC-initiating thread was already scanned in section 3 above.
    //
    // High-water-mark scanning: every other thread the collector is entitled
    // to scan is parked — either stopped at a safepoint or inside a safe
    // region — and recorded its park site (stack_cur + callee-saved register
    // snapshot park_regs) in gc_record_park() before counting itself parked,
    // which is what released gc_stw_stop_threads(). So instead of the full
    // stack reservation (512 KiB per idle task; the issue #380 collapse and
    // the #369 RSS blow-up, since the scan itself faults every page in) we
    // scan only [stack_cur, stack_hi) plus park_regs.
    //
    // Soundness: every GC reference the parked thread can use after resuming
    // is (a) in a frame at or above stack_cur — scanned; (b) in a
    // callee-saved register at park time — captured in park_regs, and any
    // spill of it by deeper frames (usleep / pthread / syscall internals) is
    // only a copy of that captured value; or (c) for a safe-region thread
    // that woke before leave parked it, a value obtained inside the region —
    // impossible by the safe-region contract (no GC heap access), so any
    // such reference existed at entry and is covered by (a)/(b) or is still
    // reachable from a scanned heap object (e.g. a channel buffer, traced
    // via GC_TAG_CHANNEL). Frames below stack_cur are dead or hold only
    // copies of (b). A thread with no park record yet (stack_cur == NULL,
    // registered but never parked) or an out-of-range record gets the old
    // conservative full-reservation scan.
    {
        pthread_t gc_self = pthread_self();
        for (int ti = 0; ti < gc_thread_stack_count; ti++) {
            GCThreadStack *t = gc_thread_stacks[ti];
            if (!t->active) continue;
            if (pthread_equal(t->thread, gc_self)) continue;
            void *tlo = t->stack_lo;
            void *thi = t->stack_hi;
            if (!tlo || !thi) continue;
            void *cur = t->stack_cur;
            if (cur && cur >= tlo && cur < thi) {
                // Scan the register snapshot saved at the park site. jmp_buf
                // SP/PC entries may be mangled on some libcs; they only add
                // noise candidates, which the conservative scan tolerates.
                long *r = (long *)&t->park_regs;
                size_t rn = sizeof(t->park_regs) / (sizeof(long));
                for (size_t ri = 0; ri < rn; ri++) {
                    gc_mark_candidate((void *)r[ri]);
                }
                tlo = cur;
            }
            tlo = (void *)(((size_t)tlo) & ~7UL);
            for (long *p = (long *)tlo; (void *)p < thi; p++) {
                gc_mark_candidate((void *)*p);
            }
        }
    }
#endif

    // 4. Scan error TLS as explicit root
    if (__pluto_current_error) {
        gc_mark_candidate(__pluto_current_error);
    }

    // 4a. Scan registered global roots (module globals holding GC refs,
    // e.g. DI singleton slots). Re-read each slot: it holds the CURRENT
    // pointer, and a zero (not yet written) is harmlessly rejected by
    // gc_mark_candidate's interval lookup.
    for (int gi = 0; gi < gc_global_root_count; gi++) {
        void **slot = (void **)gc_global_roots[gi];
        gc_mark_candidate(*slot);
    }

#ifndef PLUTO_TEST_MODE
    // 4b. Scan pending-task roots: task handles between spawn and the new
    // thread registering its stack are reachable from nowhere else.
    for (int pi = 0; pi < gc_pending_root_count; pi++) {
        gc_mark_candidate(gc_pending_roots[pi]);
    }
#endif

    // 5. Drain worklist (breadth-first trace)
    while (gc_worklist_count > 0) {
        void *obj = gc_worklist[--gc_worklist_count];
        gc_trace_object(obj);
    }

    struct timespec gc_tm;
    if (gc_log_enabled) clock_gettime(CLOCK_MONOTONIC, &gc_tm);

    // ── Sweep phase ───────────────────────────────────────────────────────
    // Keep the live-object table in sync. Rebuilding it from the survivors
    // (clear, then insert each survivor as the sweep passes it) is
    // cache-friendly and wins unless almost nothing died; individual
    // backward-shift deletes are random, cache-cold writes, so they are used
    // only when fewer than 1/8 as many objects died as survived.
    // gc_interval_count is the object count as of gc_build_intervals —
    // nothing allocates in between.
    size_t dead_count = gc_interval_count - gc_marked_count;
    int live_rebuild = dead_count * 8 >= gc_marked_count;
    if (live_rebuild) {
        size_t want = 1024;
        while (want < gc_marked_count * 4) want <<= 1;
        gc_live_reset(want);
    }
    GCHeader **pp = &gc_head;
    size_t freed_bytes = 0;
    while (*pp) {
        GCHeader *h = *pp;
        if (!h->mark) {
            *pp = h->next;
            size_t total = sizeof(GCHeader) + h->size;
            // Free array data buffer if applicable
            if (h->type_tag == GC_TAG_ARRAY && h->size >= 24) {
                long *handle = (long *)((char *)h + sizeof(GCHeader));
                void *data_ptr = (void *)handle[2];
                if (data_ptr) free(data_ptr);
            }
            // Free bytes data buffer
            if (h->type_tag == GC_TAG_BYTES && h->size >= 24) {
                long *handle = (long *)((char *)h + sizeof(GCHeader));
                void *data_ptr = (void *)handle[2];
                if (data_ptr) free(data_ptr);
            }
            // Free map buffers
            if (h->type_tag == GC_TAG_MAP && h->size >= 40) {
                long *mh = (long *)((char *)h + sizeof(GCHeader));
                if ((void *)mh[2]) free((void *)mh[2]);  // keys
                if ((void *)mh[3]) free((void *)mh[3]);  // vals
                if ((void *)mh[4]) free((void *)mh[4]);  // meta
            }
            // Free set buffers
            if (h->type_tag == GC_TAG_SET && h->size >= 32) {
                long *sh = (long *)((char *)h + sizeof(GCHeader));
                if ((void *)sh[2]) free((void *)sh[2]);  // keys
                if ((void *)sh[3]) free((void *)sh[3]);  // meta
            }
            // Free task sync resources. Test mode: slots[4] holds the FIBER
            // ID, not a TaskSync pointer — freeing it would be free(small
            // int). Nothing to release there.
#ifndef PLUTO_TEST_MODE
            if (h->type_tag == GC_TAG_TASK && h->size >= 56) {
                long *slots = (long *)((char *)h + sizeof(GCHeader));
                void *sync = (void *)slots[4];
                if (sync) {
                    pthread_mutex_destroy((pthread_mutex_t *)sync);
                    pthread_cond_destroy((pthread_cond_t *)((char *)sync + sizeof(pthread_mutex_t)));
                    free(sync);
                }
            }
#endif
            // Free channel sync + buffer
            if (h->type_tag == GC_TAG_CHANNEL && h->size >= 56) {
                long *ch = (long *)((char *)h + sizeof(GCHeader));
                void *sync = (void *)ch[0];
                void *buf  = (void *)ch[1];
                if (sync) {
#ifndef PLUTO_TEST_MODE
                    ChannelSync *cs = (ChannelSync *)sync;
                    pthread_mutex_destroy(&cs->mutex);
                    pthread_cond_destroy(&cs->not_empty);
                    pthread_cond_destroy(&cs->not_full);
#endif
                    free(sync);
                }
                if (buf) free(buf);
            }
            // Free the per-instance entity lock (hidden trailing slot; zero
            // in test mode, where __pluto_rwlock_destroy is a no-op anyway)
            if (h->type_tag == GC_TAG_ENTITY && h->size >= 8) {
                long *slots = (long *)((char *)h + sizeof(GCHeader));
                __pluto_rwlock_destroy(slots[h->size / 8 - 1]);
            }
            if (!live_rebuild) gc_live_delete((char *)h + sizeof(GCHeader));
            free(h);
            freed_bytes += total;
        } else {
            h->mark = 0;  // Clear for next cycle
            if (live_rebuild) gc_live_insert((char *)h + sizeof(GCHeader));
            pp = &h->next;
        }
    }

    // Every survivor is now in the live table.
    gc_hashed_upto = gc_head;

    struct timespec gc_ts;
    if (gc_log_enabled) clock_gettime(CLOCK_MONOTONIC, &gc_ts);

    gc_bytes_allocated -= freed_bytes;

    // Survivor-aware threshold update (policy comment at gc_threshold's
    // definition): grow the adaptive floor after a high-reclaim cycle, decay
    // it after a retention-driven one; cap it by what this process's thread
    // count justifies amortizing; then track the live set.
    size_t live = gc_bytes_allocated;
    size_t floor_cap = GC_FLOOR_BASE_CAP;
    {
        size_t nstacks = 0;
#ifdef PLUTO_TEST_MODE
        if (gc_fiber_stacks.enabled) {
            for (int fi = 0; fi < gc_fiber_stacks.count; fi++) {
                if (gc_fiber_stacks.stacks[fi].active) nstacks++;
            }
        }
#else
        nstacks = (size_t)gc_active_thread_count;
#endif
        floor_cap += nstacks * GC_FLOOR_PER_THREAD;
        if (floor_cap > GC_FLOOR_MAX) floor_cap = GC_FLOOR_MAX;
    }
    if (freed_bytes * 2 >= gc_threshold) {
        gc_adaptive_floor *= 2;
    } else {
        gc_adaptive_floor /= 2;
    }
    if (gc_adaptive_floor > floor_cap) gc_adaptive_floor = floor_cap;
    if (gc_adaptive_floor < GC_MIN_THRESHOLD) gc_adaptive_floor = GC_MIN_THRESHOLD;
    gc_threshold = live * 2;
    if (gc_threshold < gc_adaptive_floor) gc_threshold = gc_adaptive_floor;

    // Keep the interval tables and worklist allocated across cycles
    // (grow-only) to avoid per-collection malloc/free churn. Only the live
    // counts are reset; the capacities and buffers persist for process life.
    gc_interval_count = 0;
    gc_data_interval_count = 0;
    gc_worklist_count = 0;

    gc_cycle_count++;
    if (gc_log_enabled) {
        struct timespec gc_t1;
        clock_gettime(CLOCK_MONOTONIC, &gc_t1);
#define GC_US(a, b) (((b).tv_sec - (a).tv_sec) * 1000000L + ((b).tv_nsec - (a).tv_nsec) / 1000L)
        // Phase split: build = interval/lookup tables, mark = root scan +
        // trace, sweep = free unmarked objects.
        fprintf(stderr,
                "gc: #%ld live=%zu freed=%zu next_threshold=%zu pause_us=%ld"
                " build_us=%ld mark_us=%ld sweep_us=%ld\n",
                gc_cycle_count, gc_bytes_allocated, freed_bytes, gc_threshold,
                GC_US(gc_t0, gc_t1), GC_US(gc_t0, gc_tb), GC_US(gc_tb, gc_tm),
                GC_US(gc_tm, gc_ts));
#undef GC_US
    }

    gc_collecting = 0;
}

void __pluto_gc_init(void *stack_bottom) {
    // stack_bottom is the address of a slot in the entry function's frame,
    // approximating the highest address of the program stack. Besides serving
    // as the test-mode scan bound, a non-NULL value is what arms the
    // collection trigger in gc_alloc.
    gc_stack_bottom = stack_bottom;
    __pluto_register_exit_check();
#ifndef PLUTO_TEST_MODE
    // Register main thread's stack for GC root scanning
    {
        pthread_t self = pthread_self();
        void *stack_lo = NULL;
        void *stack_hi = NULL;
#ifdef __APPLE__
        stack_hi = pthread_get_stackaddr_np(self);
        size_t stack_sz = pthread_get_stacksize_np(self);
        stack_lo = (char *)stack_hi - stack_sz;
#else
        pthread_attr_t pattr;
        pthread_getattr_np(self, &pattr);
        size_t stack_sz;
        pthread_attr_getstack(&pattr, &stack_lo, &stack_sz);
        stack_hi = (char *)stack_lo + stack_sz;
        pthread_attr_destroy(&pattr);
#endif
        __pluto_gc_register_thread_stack(stack_lo, stack_hi);
        // The pthread-derived bound is exact and independent of codegen;
        // prefer it over the entry-frame anchor.
        if (stack_hi) gc_stack_bottom = stack_hi;
        (void)self;
    }
#endif
}

// ── Helper APIs for Threading (used by threading.c in Phase 2) ───────────────

#ifdef PLUTO_TEST_MODE
// Test mode helpers
void __pluto_gc_maybe_collect(void) {
    if (gc_stack_bottom && !gc_collecting
        && gc_bytes_allocated > gc_threshold) {
        __pluto_gc_collect();
    }
}

GCHeader *__pluto_gc_get_head(void) {
    return gc_head;
}

GCHeader *__pluto_gc_find_object(void *p) {
    gc_live_flush();
    return gc_find_start(p);
}

size_t __pluto_gc_bytes_allocated(void) {
    return gc_bytes_allocated;
}
#else
// Production mode helpers
int __pluto_gc_check_safepoint(void) {
    return atomic_load(&gc_safepoint_requested);
}

void __pluto_gc_maybe_collect(void) {
    // Already handled in gc_alloc, not needed externally in production mode
}

GCHeader *__pluto_gc_get_head(void) {
    return gc_head;
}

// The live table is resized by allocating threads, so lookups take the heap
// lock (counted as a safe region while waiting, like every gc_mutex wait).
GCHeader *__pluto_gc_find_object(void *p) {
    gc_heap_lock();
    gc_live_flush();
    GCHeader *h = gc_find_start(p);
    pthread_mutex_unlock(&gc_mutex);
    return h;
}

size_t __pluto_gc_bytes_allocated(void) {
    return gc_bytes_allocated;
}
#endif
