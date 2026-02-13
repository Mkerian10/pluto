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
static size_t gc_threshold = 256 * 1024;  // 256KB initial
static void *gc_stack_bottom = NULL;
#ifdef PLUTO_TEST_MODE
static int gc_collecting = 0;
#else
static atomic_int gc_collecting = 0;
#endif

// Mark worklist (raw malloc, not GC-tracked)
static void **gc_worklist = NULL;
static size_t gc_worklist_count = 0;
static size_t gc_worklist_cap = 0;

// Interval tables (rebuilt each collection)
static GCInterval *gc_intervals = NULL;
static size_t gc_interval_count = 0;
static GCDataInterval *gc_data_intervals = NULL;
static size_t gc_data_interval_count = 0;

// Thread-local storage definitions (referenced in header, defined here)
__thread void *__pluto_current_error = NULL;
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

// Fiber stack API for scheduler (test mode only)
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

// Thread registry for stop-the-world GC.
// Each spawned thread registers itself so the GC can coordinate safepoints.
#define GC_MAX_THREAD_STACKS 64
typedef struct {
    pthread_t thread;
    void *stack_lo;
    void *stack_hi;
    int active;
} GCThreadStack;
static GCThreadStack gc_thread_stacks[GC_MAX_THREAD_STACKS];
static int gc_thread_stack_count = 0;

// Safepoint-based stop-the-world state.
// GC sets gc_safepoint_requested; threads check this flag periodically and yield.
// When yielding, threads increment gc_stw_stopped and spin on gc_stw_resume.
static atomic_int gc_safepoint_requested = 0;
static volatile int gc_stw_stopped = 0;
static volatile int gc_stw_resume = 0;

// Safepoint check - called by threads at regular intervals (loop back-edges, allocations).
// If GC has requested a safepoint, the thread yields here until GC completes.
void __pluto_safepoint(void) {
    if (atomic_load(&gc_safepoint_requested) == 0) {
        return;  // Fast path - no GC pending
    }

    // GC is running - yield at this safepoint
    // Flush registers to stack so GC can scan them
    jmp_buf regs;
    setjmp(regs);
    (void)regs;  // prevent optimization

    // Signal that we've stopped
    __sync_fetch_and_add(&gc_stw_stopped, 1);

    // Spin-wait until GC is done (memory barrier to see the update)
    while (!gc_stw_resume) {
        __sync_synchronize();
    }
}

// Thread registration API for spawned tasks
void __pluto_gc_register_thread_stack(void *stack_lo, void *stack_hi) {
    pthread_mutex_lock(&gc_mutex);
    if (gc_thread_stack_count < GC_MAX_THREAD_STACKS) {
        int slot = gc_thread_stack_count++;
        gc_thread_stacks[slot].thread = pthread_self();
        gc_thread_stacks[slot].stack_lo = stack_lo;
        gc_thread_stacks[slot].stack_hi = stack_hi;
        gc_thread_stacks[slot].active = 1;
    }
    pthread_mutex_unlock(&gc_mutex);
}

void __pluto_gc_deregister_thread_stack(void) {
    pthread_t self = pthread_self();
    pthread_mutex_lock(&gc_mutex);
    for (int i = 0; i < gc_thread_stack_count; i++) {
        if (pthread_equal(gc_thread_stacks[i].thread, self)) {
            gc_thread_stacks[i].active = 0;
            break;
        }
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
#endif

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
// Stop all registered task threads via safepoint polling.
// Sets the global safepoint flag and waits for all threads to yield.
// Returns the number of threads that were stopped. Caller must call gc_stw_resume_threads() after GC.
static int gc_stw_stop_threads(void) {
    int count = 0;
    gc_stw_stopped = 0;
    gc_stw_resume = 0;
    __sync_synchronize();  // memory barrier

    // Count active threads (excluding self)
    pthread_t self = pthread_self();
    for (int i = 0; i < gc_thread_stack_count; i++) {
        if (!gc_thread_stacks[i].active) continue;
        if (pthread_equal(gc_thread_stacks[i].thread, self)) continue;  // skip self
        count++;
    }

    if (count > 0) {
        // Request all threads to stop at their next safepoint
        atomic_store(&gc_safepoint_requested, 1);
        __sync_synchronize();

        // Wait for all threads to acknowledge (NO TIMEOUT - they WILL hit a safepoint)
        while (__sync_fetch_and_add(&gc_stw_stopped, 0) < count) {
            __sync_synchronize();
            usleep(100);  // yield CPU, don't spin
        }
    }
    return count;
}

static void gc_stw_resume_threads(void) {
    gc_stw_resume = 1;
    __sync_synchronize();  // ensure visibility
    atomic_store(&gc_safepoint_requested, 0);  // Clear safepoint request
    __sync_synchronize();
}

void *gc_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count) {
    pthread_mutex_lock(&gc_mutex);
    if (gc_stack_bottom
        && gc_bytes_allocated + user_size + sizeof(GCHeader) > gc_threshold) {
        // Atomic test-and-set: only one thread wins the race to initiate GC
        int expected = 0;
        if (atomic_compare_exchange_strong(&gc_collecting, &expected, 1)) {
            // This thread won - run GC
            int stopped = gc_stw_stop_threads();
            __pluto_gc_collect();  // This will set gc_collecting back to 0
            if (stopped > 0) gc_stw_resume_threads();
        } else {
            // Another thread is collecting - wait for it to finish
            // Note: Cannot use usleep() here because SA_RESTART in signal handler
            // would cause usleep() to restart when SIGUSR1 arrives, creating deadlock.
            pthread_mutex_unlock(&gc_mutex);
            while (atomic_load(&gc_collecting) == 1) {
                // Yield CPU to let GC thread make progress
                __sync_synchronize();  // memory barrier
            }
            pthread_mutex_lock(&gc_mutex);
        }
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

static void gc_build_intervals(void) {
    // Count objects
    size_t count = 0;
    size_t data_buf_count = 0;
    for (GCHeader *h = gc_head; h; h = h->next) {
        count++;
        if (h->type_tag == GC_TAG_ARRAY) data_buf_count++;
        else if (h->type_tag == GC_TAG_BYTES) data_buf_count++;
        else if (h->type_tag == GC_TAG_MAP) data_buf_count += 3;  // keys, vals, meta
        else if (h->type_tag == GC_TAG_SET) data_buf_count += 2;  // keys, meta
    }

    gc_intervals = (GCInterval *)malloc(count * sizeof(GCInterval));
    gc_interval_count = count;
    gc_data_intervals = (GCDataInterval *)malloc(data_buf_count * sizeof(GCDataInterval));
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

    qsort(gc_intervals, gc_interval_count, sizeof(GCInterval), gc_interval_cmp);
    if (gc_data_interval_count > 0) {
        qsort(gc_data_intervals, gc_data_interval_count, sizeof(GCDataInterval), gc_data_interval_cmp);
    }
}

// Binary search: find GC object containing candidate pointer
static GCHeader *gc_find_object(void *candidate) {
    if (gc_interval_count == 0) return NULL;
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

    // Build interval tables
    gc_build_intervals();

    // Reset worklist
    gc_worklist_count = 0;

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
    // In test mode, we use gc_stack_bottom (always main thread, single-threaded).
    {
        void *stack_top;
        volatile long anchor = 0;
        (void)anchor;
        stack_top = (void *)&anchor;

#ifndef PLUTO_TEST_MODE
        // Find this thread's registered stack entry to get the correct stack_hi.
        // Without this, a task thread would scan from its stack to gc_stack_bottom
        // (the main thread's stack), crossing unmapped memory → SEGFAULT.
        pthread_t self = pthread_self();
        void *hi = gc_stack_bottom;  // fallback for main thread
        for (int i = 0; i < gc_thread_stack_count; i++) {
            if (pthread_equal(gc_thread_stacks[i].thread, self)) {
                hi = gc_thread_stacks[i].stack_hi;
                break;
            }
        }
        void *lo = stack_top;
        // On most platforms stacks grow down, so stack_top < stack_hi.
        // Handle either direction just in case.
        if (lo > hi) { void *tmp = lo; lo = hi; hi = tmp; }
#else
        void *lo = stack_top < gc_stack_bottom ? stack_top : gc_stack_bottom;
        void *hi = stack_top < gc_stack_bottom ? gc_stack_bottom : stack_top;
#endif
        lo = (void *)(((size_t)lo) & ~7UL);
        for (long *p = (long *)lo; (void *)p < hi; p++) {
            gc_mark_candidate((void *)*p);
        }
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
            // Scan the entire fiber stack allocation
            void *flo = (void *)(((size_t)base) & ~7UL);
            void *fhi = (void *)(base + sz);
            for (long *p = (long *)flo; (void *)p < fhi; p++) {
                gc_mark_candidate((void *)*p);
            }
        }
    }
#endif

#ifndef PLUTO_TEST_MODE
    // 3c. Scan all OTHER registered thread stacks as additional GC roots.
    // The GC-initiating thread was already scanned in section 3 above.
    // Stopped threads (paused at safepoints) have their full register state
    // saved on their stacks via setjmp in the safepoint handler.
    {
        pthread_t gc_self = pthread_self();
        for (int ti = 0; ti < gc_thread_stack_count; ti++) {
            if (!gc_thread_stacks[ti].active) continue;
            if (pthread_equal(gc_thread_stacks[ti].thread, gc_self)) continue;
            void *tlo = gc_thread_stacks[ti].stack_lo;
            void *thi = gc_thread_stacks[ti].stack_hi;
            if (!tlo || !thi) continue;
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

    // 5. Drain worklist (breadth-first trace)
    while (gc_worklist_count > 0) {
        void *obj = gc_worklist[--gc_worklist_count];
        gc_trace_object(obj);
    }

    // ── Sweep phase ───────────────────────────────────────────────────────
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
            // Free task sync resources
            if (h->type_tag == GC_TAG_TASK && h->size >= 56) {
                long *slots = (long *)((char *)h + sizeof(GCHeader));
                void *sync = (void *)slots[4];
                if (sync) {
#ifndef PLUTO_TEST_MODE
                    pthread_mutex_destroy((pthread_mutex_t *)sync);
                    pthread_cond_destroy((pthread_cond_t *)((char *)sync + sizeof(pthread_mutex_t)));
#endif
                    free(sync);
                }
            }
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
            free(h);
            freed_bytes += total;
        } else {
            h->mark = 0;  // Clear for next cycle
            pp = &h->next;
        }
    }

    gc_bytes_allocated -= freed_bytes;
    size_t surviving = gc_bytes_allocated;
    gc_threshold = surviving * 2;
    if (gc_threshold < 256 * 1024) gc_threshold = 256 * 1024;

    // Free interval tables and worklist
    free(gc_intervals);
    gc_intervals = NULL;
    gc_interval_count = 0;
    free(gc_data_intervals);
    gc_data_intervals = NULL;
    gc_data_interval_count = 0;
    free(gc_worklist);
    gc_worklist = NULL;
    gc_worklist_count = 0;
    gc_worklist_cap = 0;

    gc_collecting = 0;
}

void __pluto_gc_init(void *stack_bottom) {
    gc_stack_bottom = stack_bottom;
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
        gc_thread_stacks[0].thread = self;
        gc_thread_stacks[0].stack_lo = stack_lo;
        gc_thread_stacks[0].stack_hi = stack_hi;
        gc_thread_stacks[0].active = 1;
        gc_thread_stack_count = 1;
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

size_t __pluto_gc_bytes_allocated(void) {
    return gc_bytes_allocated;
}
#endif
