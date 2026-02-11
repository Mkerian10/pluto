#define _XOPEN_SOURCE 700
#define _GNU_SOURCE
#ifdef __APPLE__
#define _DARWIN_C_SOURCE
#endif
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <setjmp.h>
#include <time.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <unistd.h>
#include <signal.h>
#include <errno.h>
#include <sys/stat.h>
#include <dirent.h>
#include <fcntl.h>
#include <limits.h>
#include <math.h>
#ifndef PLUTO_TEST_MODE
#include <pthread.h>
#include <stdatomic.h>
#endif
#ifdef PLUTO_TEST_MODE
#include <ucontext.h>
#endif

// ── Forward declarations ─────────────────────────────────────────────────────
void __pluto_raise_error(void *error_obj);

// ── GC Infrastructure ─────────────────────────────────────────────────────────

// Type tags for GC objects
#define GC_TAG_OBJECT 0   // class, enum, closure, error, DI singleton
#define GC_TAG_STRING 1   // no child pointers
#define GC_TAG_ARRAY  2   // handle [len][cap][data_ptr]; data buffer freed on sweep
#define GC_TAG_TRAIT  3   // [data_ptr][vtable_ptr]; trace data_ptr only
#define GC_TAG_MAP   4   // [count][cap][keys_ptr][vals_ptr][meta_ptr]
#define GC_TAG_SET   5   // [count][cap][keys_ptr][meta_ptr]
#define GC_TAG_JSON  6   // (reserved, formerly JsonNode)
#define GC_TAG_TASK    7   // [closure][result][error][done][sync_ptr][detached][cancelled]
#define GC_TAG_BYTES   8   // [len][cap][data_ptr]; 1 byte per element
#define GC_TAG_CHANNEL 9   // [sync_ptr][buf_ptr][capacity][count][head][tail][closed]

#ifndef PLUTO_TEST_MODE
typedef struct {
    pthread_mutex_t mutex;
    pthread_cond_t not_empty;
    pthread_cond_t not_full;
} ChannelSync;
#endif

typedef struct GCHeader {
    struct GCHeader *next;    // 8B: linked list of all GC objects
    uint32_t size;            // 4B: user data size in bytes
    uint8_t  mark;            // 1B: 0=unmarked, 1=marked
    uint8_t  type_tag;        // 1B: object kind
    uint16_t field_count;     // 2B: number of 8-byte slots to scan
} GCHeader;

// Interval for binary-search pointer lookup
typedef struct { void *start; void *end; GCHeader *header; } GCInterval;
// Array data buffer interval
typedef struct { void *start; void *end; void *array_handle; } GCDataInterval;

// Global GC state
static GCHeader *gc_head = NULL;
static size_t gc_bytes_allocated = 0;
static size_t gc_threshold = 256 * 1024;  // 256KB initial
static void *gc_stack_bottom = NULL;
static int gc_collecting = 0;

// Mark worklist (raw malloc, not GC-tracked)
static void **gc_worklist = NULL;
static size_t gc_worklist_count = 0;
static size_t gc_worklist_cap = 0;

// Interval tables (rebuilt each collection)
static GCInterval *gc_intervals = NULL;
static size_t gc_interval_count = 0;
static GCDataInterval *gc_data_intervals = NULL;
static size_t gc_data_interval_count = 0;

// Forward declarations
void __pluto_gc_collect(void);
void *__pluto_array_new(long cap);
void __pluto_array_push(void *handle, long value);

// Error handling — thread-local so each thread has its own error state
__thread void *__pluto_current_error = NULL;

// Task handle — thread-local pointer to current task (NULL on main thread)
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
#endif

// GC thread safety
#ifndef PLUTO_TEST_MODE
static pthread_mutex_t gc_mutex = PTHREAD_MUTEX_INITIALIZER;
static atomic_int __pluto_active_tasks = 0;

// Thread registry for signal-based stop-the-world GC.
// Each spawned thread registers itself so the GC can send SIGUSR1 to pause it.
#define GC_MAX_THREAD_STACKS 64
typedef struct {
    pthread_t thread;
    void *stack_lo;
    void *stack_hi;
    int active;
} GCThreadStack;
static GCThreadStack gc_thread_stacks[GC_MAX_THREAD_STACKS];
static int gc_thread_stack_count = 0;

// Signal-based stop-the-world state.
// GC sends SIGUSR1 to each thread; the handler flushes registers (setjmp),
// increments gc_stw_stopped, and spins until gc_stw_resume is set.
static volatile int gc_stw_stopped = 0;
static volatile int gc_stw_resume = 0;

static void gc_stw_signal_handler(int sig) {
    (void)sig;
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

static void gc_stw_install_handler(void) {
    struct sigaction sa;
    memset(&sa, 0, sizeof(sa));
    sa.sa_handler = gc_stw_signal_handler;
    sa.sa_flags = SA_RESTART;
    sigfillset(&sa.sa_mask);  // block all signals during handler
    sigaction(SIGUSR1, &sa, NULL);
}
#endif

static inline GCHeader *gc_get_header(void *user_ptr) {
    return (GCHeader *)((char *)user_ptr - sizeof(GCHeader));
}

#ifdef PLUTO_TEST_MODE
static void *gc_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count) {
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
// Stop all registered task threads via SIGUSR1 and wait for them to acknowledge.
// Returns the number of threads that were stopped. Caller must call gc_stw_resume_threads() after GC.
// Stop all registered task threads via SIGUSR1 and wait for them to acknowledge.
// Returns the number of threads that were stopped. Caller must call gc_stw_resume_threads() after GC.
static int gc_stw_stop_threads(void) {
    int count = 0;
    gc_stw_stopped = 0;
    gc_stw_resume = 0;
    __sync_synchronize();  // memory barrier

    pthread_t self = pthread_self();
    for (int i = 0; i < gc_thread_stack_count; i++) {
        if (!gc_thread_stacks[i].active) continue;
        if (pthread_equal(gc_thread_stacks[i].thread, self)) continue;  // skip self
        pthread_kill(gc_thread_stacks[i].thread, SIGUSR1);
        count++;
    }

    if (count > 0) {
        // Wait for all threads to acknowledge (with timeout)
        int spins = 0;
        while (__sync_fetch_and_add(&gc_stw_stopped, 0) < count) {
            __sync_synchronize();
            if (++spins > 1000000) break;  // ~1s timeout — give up
        }
    }
    return count;
}

static void gc_stw_resume_threads(void) {
    gc_stw_resume = 1;
    __sync_synchronize();  // ensure visibility
}

static void *gc_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count) {
    pthread_mutex_lock(&gc_mutex);
    if (gc_stack_bottom && !gc_collecting
        && gc_bytes_allocated + user_size + sizeof(GCHeader) > gc_threshold) {
        // Stop all other task threads via SIGUSR1 so we can safely scan their stacks
        int stopped = gc_stw_stop_threads();
        __pluto_gc_collect();
        if (stopped > 0) gc_stw_resume_threads();
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
    // Stopped threads (paused by SIGUSR1) have their full register state
    // saved on their stacks via the kernel signal frame + setjmp in the handler.
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

void __pluto_gc_init(void) {
    volatile long anchor = 0;
    (void)anchor;
    gc_stack_bottom = (void *)&anchor;
#ifndef PLUTO_TEST_MODE
    gc_stw_install_handler();
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

// ── Print functions ───────────────────────────────────────────────────────────

void __pluto_print_int(long value) {
    printf("%ld\n", value);
}

void __pluto_print_float(double value) {
    printf("%f\n", value);
}

void __pluto_print_string(void *header) {
    const char *data = (const char *)header + 8;
    printf("%s\n", data);
}

void __pluto_print_bool(int value) {
    printf("%s\n", value ? "true" : "false");
}

void __pluto_print_string_no_newline(void *header) {
    const char *data = (const char *)header + 8;
    printf("%s", data);
}

// ── Memory allocation ─────────────────────────────────────────────────────────

void *__pluto_alloc(long size) {
    if (size == 0) size = 8;
    uint16_t field_count = (uint16_t)(size / 8);
    return gc_alloc((size_t)size, GC_TAG_OBJECT, field_count);
}

void *__pluto_trait_wrap(long data_ptr, long vtable_ptr) {
    long *handle = (long *)gc_alloc(16, GC_TAG_TRAIT, 2);
    handle[0] = data_ptr;
    handle[1] = vtable_ptr;
    return handle;
}

// ── String functions ──────────────────────────────────────────────────────────

void *__pluto_string_new(const char *data, long len) {
    size_t alloc_size = 8 + len + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = len;
    memcpy((char *)header + 8, data, len);
    ((char *)header)[8 + len] = '\0';
    return header;
}

void *__pluto_io_read_line(void) {
    char *buf = NULL;
    size_t cap = 0;
    ssize_t len = getline(&buf, &cap, stdin);
    if (len < 0) {
        free(buf);
        return __pluto_string_new("", 0);
    }
    while (len > 0 && (buf[len - 1] == '\n' || buf[len - 1] == '\r')) {
        len--;
    }
    void *result = __pluto_string_new(buf, len);
    free(buf);
    return result;
}

void *__pluto_string_concat(void *a, void *b) {
    long len_a = *(long *)a;
    long len_b = *(long *)b;
    if (len_a > LONG_MAX - len_b) {
        fprintf(stderr, "pluto: string concatenation overflow\n");
        exit(1);
    }
    long total = len_a + len_b;
    size_t alloc_size = 8 + total + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = total;
    memcpy((char *)header + 8, (char *)a + 8, len_a);
    memcpy((char *)header + 8 + len_a, (char *)b + 8, len_b);
    ((char *)header)[8 + total] = '\0';
    return header;
}

int __pluto_string_eq(void *a, void *b) {
    long len_a = *(long *)a;
    long len_b = *(long *)b;
    if (len_a != len_b) return 0;
    return memcmp((char *)a + 8, (char *)b + 8, len_a) == 0 ? 1 : 0;
}

long __pluto_string_len(void *s) {
    return *(long *)s;
}

// ── Array runtime functions ───────────────────────────────────────────────────
// Handle layout (24 bytes): [len: long] [cap: long] [data_ptr: long*]

void *__pluto_array_new(long cap) {
    long *handle = (long *)gc_alloc(24, GC_TAG_ARRAY, 3);
    handle[0] = 0;   // len
    handle[1] = cap;  // cap
    // Data buffer is NOT GC-tracked — raw malloc/realloc
    long *data = (long *)malloc(cap * 8);
    if (!data) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    handle[2] = (long)data;
    return handle;
}

void __pluto_array_push(void *handle, long value) {
    long *h = (long *)handle;
    long len = h[0];
    long cap = h[1];
    long *data = (long *)h[2];
    if (len == cap) {
        if (cap > LONG_MAX / 2) {
            fprintf(stderr, "pluto: array capacity overflow\n");
            exit(1);
        }
        cap = cap * 2;
        if (cap == 0) cap = 4;
        data = (long *)realloc(data, cap * 8);
        if (!data) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
        h[1] = cap;
        h[2] = (long)data;
    }
    data[len] = value;
    h[0] = len + 1;
}

long __pluto_array_get(void *handle, long index) {
    long *h = (long *)handle;
    long len = h[0];
    if (index < 0 || index >= len) {
        fprintf(stderr, "pluto: array index out of bounds: index %ld, length %ld\n", index, len);
        exit(1);
    }
    long *data = (long *)h[2];
    return data[index];
}

void __pluto_array_set(void *handle, long index, long value) {
    long *h = (long *)handle;
    long len = h[0];
    if (index < 0 || index >= len) {
        fprintf(stderr, "pluto: array index out of bounds: index %ld, length %ld\n", index, len);
        exit(1);
    }
    long *data = (long *)h[2];
    data[index] = value;
}

long __pluto_array_len(void *handle) {
    return ((long *)handle)[0];
}

long __pluto_array_pop(void *handle) {
    long *h = (long *)handle;
    long len = h[0];
    if (len == 0) {
        fprintf(stderr, "pluto: pop from empty array\n");
        exit(1);
    }
    long *data = (long *)h[2];
    h[0] = len - 1;
    return data[len - 1];
}

long __pluto_array_last(void *handle) {
    long *h = (long *)handle;
    long len = h[0];
    if (len == 0) {
        fprintf(stderr, "pluto: last() on empty array\n");
        exit(1);
    }
    long *data = (long *)h[2];
    return data[len - 1];
}

long __pluto_array_first(void *handle) {
    long *h = (long *)handle;
    long len = h[0];
    if (len == 0) {
        fprintf(stderr, "pluto: first() on empty array\n");
        exit(1);
    }
    long *data = (long *)h[2];
    return data[0];
}

void __pluto_array_clear(void *handle) {
    ((long *)handle)[0] = 0;
}

long __pluto_array_remove_at(void *handle, long index) {
    long *h = (long *)handle;
    long len = h[0];
    if (index < 0 || index >= len) {
        fprintf(stderr, "pluto: array remove_at index out of bounds: index %ld, length %ld\n", index, len);
        exit(1);
    }
    long *data = (long *)h[2];
    long removed = data[index];
    for (long i = index; i < len - 1; i++) {
        data[i] = data[i + 1];
    }
    h[0] = len - 1;
    return removed;
}

void __pluto_array_insert_at(void *handle, long index, long value) {
    long *h = (long *)handle;
    long len = h[0];
    if (index < 0 || index > len) {
        fprintf(stderr, "pluto: array insert_at index out of bounds: index %ld, length %ld\n", index, len);
        exit(1);
    }
    long cap = h[1];
    long *data = (long *)h[2];
    if (len == cap) {
        cap = cap * 2;
        if (cap == 0) cap = 4;
        data = (long *)realloc(data, cap * 8);
        if (!data) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
        h[1] = cap;
        h[2] = (long)data;
    }
    for (long i = len; i > index; i--) {
        data[i] = data[i - 1];
    }
    data[index] = value;
    h[0] = len + 1;
}

void *__pluto_array_slice(void *handle, long start, long end) {
    long *h = (long *)handle;
    long len = h[0];
    if (start < 0) start = 0;
    if (end > len) end = len;
    if (start > end) start = end;
    long new_len = end - start;
    long *data = (long *)h[2];
    void *new_handle = __pluto_array_new(new_len > 0 ? new_len : 1);
    long *nh = (long *)new_handle;
    long *new_data = (long *)nh[2];
    for (long i = 0; i < new_len; i++) {
        new_data[i] = data[start + i];
    }
    nh[0] = new_len;
    return new_handle;
}

void __pluto_array_reverse(void *handle) {
    long *h = (long *)handle;
    long len = h[0];
    long *data = (long *)h[2];
    for (long i = 0; i < len / 2; i++) {
        long tmp = data[i];
        data[i] = data[len - 1 - i];
        data[len - 1 - i] = tmp;
    }
}

long __pluto_array_contains(void *handle, long value, long type_tag) {
    long *h = (long *)handle;
    long len = h[0];
    long *data = (long *)h[2];
    for (long i = 0; i < len; i++) {
        if (type_tag == 3) { // string
            if (__pluto_string_eq((void *)data[i], (void *)value)) return 1;
        } else {
            if (data[i] == value) return 1;
        }
    }
    return 0;
}

long __pluto_array_index_of(void *handle, long value, long type_tag) {
    long *h = (long *)handle;
    long len = h[0];
    long *data = (long *)h[2];
    for (long i = 0; i < len; i++) {
        if (type_tag == 3) { // string
            if (__pluto_string_eq((void *)data[i], (void *)value)) return i;
        } else {
            if (data[i] == value) return i;
        }
    }
    return -1;
}

// ── Bytes runtime functions ───────────────────────────────────────────────────
// Handle layout (24 bytes): [len: long] [cap: long] [data_ptr: unsigned char*]

long __pluto_bytes_new(void) {
    long *handle = (long *)gc_alloc(24, GC_TAG_BYTES, 3);
    handle[0] = 0;   // len
    handle[1] = 16;  // cap (initial)
    unsigned char *data = (unsigned char *)malloc(16);
    if (!data) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    handle[2] = (long)data;
    return (long)handle;
}

void __pluto_bytes_push(long handle, long value) {
    long *h = (long *)handle;
    long len = h[0];
    long cap = h[1];
    unsigned char *data = (unsigned char *)h[2];
    if (len == cap) {
        if (cap > LONG_MAX / 2) {
            fprintf(stderr, "pluto: bytes capacity overflow\n");
            exit(1);
        }
        cap = cap * 2;
        if (cap == 0) cap = 16;
        data = (unsigned char *)realloc(data, cap);
        if (!data) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
        h[1] = cap;
        h[2] = (long)data;
    }
    data[len] = (unsigned char)(value & 0xFF);
    h[0] = len + 1;
}

long __pluto_bytes_get(long handle, long index) {
    long *h = (long *)handle;
    long len = h[0];
    if (index < 0 || index >= len) {
        fprintf(stderr, "pluto: bytes index out of bounds: index %ld, length %ld\n", index, len);
        exit(1);
    }
    unsigned char *data = (unsigned char *)h[2];
    return (long)data[index];
}

void __pluto_bytes_set(long handle, long index, long value) {
    long *h = (long *)handle;
    long len = h[0];
    if (index < 0 || index >= len) {
        fprintf(stderr, "pluto: bytes index out of bounds: index %ld, length %ld\n", index, len);
        exit(1);
    }
    unsigned char *data = (unsigned char *)h[2];
    data[index] = (unsigned char)(value & 0xFF);
}

long __pluto_bytes_len(long handle) {
    return ((long *)handle)[0];
}

long __pluto_bytes_to_string(long handle) {
    long *h = (long *)handle;
    long len = h[0];
    unsigned char *data = (unsigned char *)h[2];
    size_t alloc_size = 8 + len + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = len;
    memcpy((char *)header + 8, data, len);
    ((char *)header)[8 + len] = '\0';
    return (long)header;
}

long __pluto_string_to_bytes(long str_handle) {
    void *s = (void *)str_handle;
    long len = *(long *)s;
    const char *str_data = (const char *)s + 8;
    long *handle = (long *)gc_alloc(24, GC_TAG_BYTES, 3);
    long cap = len > 16 ? len : 16;
    handle[0] = len;
    handle[1] = cap;
    unsigned char *data = (unsigned char *)malloc(cap);
    if (!data) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    memcpy(data, str_data, len);
    handle[2] = (long)data;
    return (long)handle;
}

// ── String utility functions ──────────────────────────────────────────────────

void *__pluto_string_substring(void *s, long start, long len) {
    long slen = *(long *)s;
    const char *data = (const char *)s + 8;
    if (start < 0) start = 0;
    if (start > slen) start = slen;
    if (len < 0) len = 0;
    if (start + len > slen) len = slen - start;
    size_t alloc_size = 8 + len + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = len;
    memcpy((char *)header + 8, data + start, len);
    ((char *)header)[8 + len] = '\0';
    return header;
}

long __pluto_string_contains(void *haystack, void *needle) {
    long hlen = *(long *)haystack;
    long nlen = *(long *)needle;
    if (nlen == 0) return 1;
    if (nlen > hlen) return 0;
    const char *hdata = (const char *)haystack + 8;
    const char *ndata = (const char *)needle + 8;
    return memmem(hdata, hlen, ndata, nlen) != NULL ? 1 : 0;
}

long __pluto_string_starts_with(void *s, void *prefix) {
    long slen = *(long *)s;
    long plen = *(long *)prefix;
    if (plen == 0) return 1;
    if (plen > slen) return 0;
    return memcmp((const char *)s + 8, (const char *)prefix + 8, plen) == 0 ? 1 : 0;
}

long __pluto_string_ends_with(void *s, void *suffix) {
    long slen = *(long *)s;
    long sfxlen = *(long *)suffix;
    if (sfxlen == 0) return 1;
    if (sfxlen > slen) return 0;
    return memcmp((const char *)s + 8 + slen - sfxlen, (const char *)suffix + 8, sfxlen) == 0 ? 1 : 0;
}

long __pluto_string_index_of(void *haystack, void *needle) {
    long hlen = *(long *)haystack;
    long nlen = *(long *)needle;
    if (nlen == 0) return 0;
    if (nlen > hlen) return -1;
    const char *hdata = (const char *)haystack + 8;
    const char *ndata = (const char *)needle + 8;
    const char *found = (const char *)memmem(hdata, hlen, ndata, nlen);
    if (!found) return -1;
    return (long)(found - hdata);
}

void *__pluto_string_trim(void *s) {
    long slen = *(long *)s;
    const char *data = (const char *)s + 8;
    long start = 0;
    long end = slen;
    while (start < end && (data[start] == ' ' || data[start] == '\t' || data[start] == '\n' || data[start] == '\r')) start++;
    while (end > start && (data[end-1] == ' ' || data[end-1] == '\t' || data[end-1] == '\n' || data[end-1] == '\r')) end--;
    long newlen = end - start;
    size_t alloc_size = 8 + newlen + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = newlen;
    memcpy((char *)header + 8, data + start, newlen);
    ((char *)header)[8 + newlen] = '\0';
    return header;
}

void *__pluto_string_to_upper(void *s) {
    long slen = *(long *)s;
    const char *data = (const char *)s + 8;
    size_t alloc_size = 8 + slen + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = slen;
    char *out = (char *)header + 8;
    for (long i = 0; i < slen; i++) {
        out[i] = (char)toupper((unsigned char)data[i]);
    }
    out[slen] = '\0';
    return header;
}

void *__pluto_string_to_lower(void *s) {
    long slen = *(long *)s;
    const char *data = (const char *)s + 8;
    size_t alloc_size = 8 + slen + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = slen;
    char *out = (char *)header + 8;
    for (long i = 0; i < slen; i++) {
        out[i] = (char)tolower((unsigned char)data[i]);
    }
    out[slen] = '\0';
    return header;
}

void *__pluto_string_replace(void *s, void *old, void *new_str) {
    long slen = *(long *)s;
    long olen = *(long *)old;
    long nlen = *(long *)new_str;
    const char *sdata = (const char *)s + 8;
    const char *odata = (const char *)old + 8;
    const char *ndata = (const char *)new_str + 8;
    if (olen == 0) {
        size_t alloc_size = 8 + slen + 1;
        void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
        *(long *)header = slen;
        memcpy((char *)header + 8, sdata, slen);
        ((char *)header)[8 + slen] = '\0';
        return header;
    }
    long count = 0;
    const char *p = sdata;
    long remaining = slen;
    while (remaining >= olen) {
        const char *found = (const char *)memmem(p, remaining, odata, olen);
        if (!found) break;
        count++;
        remaining -= (found - p) + olen;
        p = found + olen;
    }
    if (nlen > olen && count > 0) {
        if (count > (LONG_MAX - slen) / (nlen - olen)) {
            fprintf(stderr, "pluto: string replace overflow\n");
            exit(1);
        }
    }
    long newlen = slen + count * (nlen - olen);
    size_t alloc_size = 8 + newlen + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = newlen;
    char *out = (char *)header + 8;
    p = sdata;
    remaining = slen;
    while (remaining >= olen) {
        const char *found = (const char *)memmem(p, remaining, odata, olen);
        if (!found) break;
        long before = found - p;
        memcpy(out, p, before);
        out += before;
        memcpy(out, ndata, nlen);
        out += nlen;
        remaining -= before + olen;
        p = found + olen;
    }
    memcpy(out, p, remaining);
    out[remaining] = '\0';
    return header;
}

void *__pluto_string_split(void *s, void *delim) {
    long slen = *(long *)s;
    long dlen = *(long *)delim;
    const char *sdata = (const char *)s + 8;
    const char *ddata = (const char *)delim + 8;
    void *arr = __pluto_array_new(4);
    if (dlen == 0) {
        for (long i = 0; i < slen; i++) {
            void *ch = gc_alloc(8 + 1 + 1, GC_TAG_STRING, 0);
            *(long *)ch = 1;
            ((char *)ch)[8] = sdata[i];
            ((char *)ch)[9] = '\0';
            __pluto_array_push(arr, (long)ch);
        }
        return arr;
    }
    const char *p = sdata;
    long remaining = slen;
    while (1) {
        if (remaining < dlen) {
            size_t alloc_size = 8 + remaining + 1;
            void *seg = gc_alloc(alloc_size, GC_TAG_STRING, 0);
            *(long *)seg = remaining;
            memcpy((char *)seg + 8, p, remaining);
            ((char *)seg)[8 + remaining] = '\0';
            __pluto_array_push(arr, (long)seg);
            break;
        }
        const char *found = (const char *)memmem(p, remaining, ddata, dlen);
        if (!found) {
            size_t alloc_size = 8 + remaining + 1;
            void *seg = gc_alloc(alloc_size, GC_TAG_STRING, 0);
            *(long *)seg = remaining;
            memcpy((char *)seg + 8, p, remaining);
            ((char *)seg)[8 + remaining] = '\0';
            __pluto_array_push(arr, (long)seg);
            break;
        }
        long seglen = found - p;
        size_t alloc_size = 8 + seglen + 1;
        void *seg = gc_alloc(alloc_size, GC_TAG_STRING, 0);
        *(long *)seg = seglen;
        memcpy((char *)seg + 8, p, seglen);
        ((char *)seg)[8 + seglen] = '\0';
        __pluto_array_push(arr, (long)seg);
        remaining -= seglen + dlen;
        p = found + dlen;
    }
    return arr;
}

void *__pluto_string_char_at(void *s, long index) {
    long slen = *(long *)s;
    if (index < 0 || index >= slen) {
        fprintf(stderr, "pluto: string index out of bounds: index %ld, length %ld\n", index, slen);
        exit(1);
    }
    const char *data = (const char *)s + 8;
    void *header = gc_alloc(8 + 1 + 1, GC_TAG_STRING, 0);
    *(long *)header = 1;
    ((char *)header)[8] = data[index];
    ((char *)header)[9] = '\0';
    return header;
}

long __pluto_string_byte_at(void *s, long index) {
    long slen = *(long *)s;
    if (index < 0 || index >= slen) {
        fprintf(stderr, "pluto: string byte_at index out of bounds: index %ld, length %ld\n", index, slen);
        exit(1);
    }
    const char *data = (const char *)s + 8;
    return (long)(unsigned char)data[index];
}

void *__pluto_string_format_float(double value) {
    int len = snprintf(NULL, 0, "%g", value);
    size_t alloc_size = 8 + len + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = len;
    snprintf((char *)header + 8, len + 1, "%g", value);
    return header;
}

void *__pluto_string_to_int(void *s) {
    long slen = *(long *)s;
    const char *data = (const char *)s + 8;
    char *tmp = (char *)malloc(slen + 1);
    memcpy(tmp, data, slen);
    tmp[slen] = '\0';
    // Skip leading/trailing whitespace
    char *start = tmp;
    while (*start == ' ' || *start == '\t' || *start == '\n' || *start == '\r') start++;
    char *end_ptr;
    long result = strtol(start, &end_ptr, 10);
    // Skip trailing whitespace
    while (*end_ptr == ' ' || *end_ptr == '\t' || *end_ptr == '\n' || *end_ptr == '\r') end_ptr++;
    if (start == end_ptr || *end_ptr != '\0') {
        free(tmp);
        // Return none (null pointer)
        return (void *)0;
    }
    free(tmp);
    // Return boxed int value (nullable representation)
    void *obj = gc_alloc(8, GC_TAG_OBJECT, 0);
    *(long *)obj = result;
    return obj;
}

void *__pluto_string_to_float(void *s) {
    long slen = *(long *)s;
    const char *data = (const char *)s + 8;
    char *tmp = (char *)malloc(slen + 1);
    memcpy(tmp, data, slen);
    tmp[slen] = '\0';
    // Skip leading/trailing whitespace
    char *start = tmp;
    while (*start == ' ' || *start == '\t' || *start == '\n' || *start == '\r') start++;
    char *end_ptr;
    double result = strtod(start, &end_ptr);
    // Skip trailing whitespace
    while (*end_ptr == ' ' || *end_ptr == '\t' || *end_ptr == '\n' || *end_ptr == '\r') end_ptr++;
    if (start == end_ptr || *end_ptr != '\0') {
        free(tmp);
        // Return none (null pointer)
        return (void *)0;
    }
    free(tmp);
    // Return boxed float value (nullable representation: float stored as bitcast i64)
    void *obj = gc_alloc(8, GC_TAG_OBJECT, 0);
    memcpy(obj, &result, 8);
    return obj;
}

void *__pluto_string_trim_start(void *s) {
    long slen = *(long *)s;
    const char *data = (const char *)s + 8;
    // Skip leading whitespace
    long start_idx = 0;
    while (start_idx < slen && (data[start_idx] == ' ' || data[start_idx] == '\t' || data[start_idx] == '\n' || data[start_idx] == '\r')) {
        start_idx++;
    }
    long new_len = slen - start_idx;
    void *obj = gc_alloc(8 + new_len + 1, GC_TAG_STRING, 0);
    *(long *)obj = new_len;
    memcpy((char *)obj + 8, data + start_idx, new_len);
    ((char *)obj + 8)[new_len] = '\0';
    return obj;
}

void *__pluto_string_trim_end(void *s) {
    long slen = *(long *)s;
    const char *data = (const char *)s + 8;
    // Skip trailing whitespace
    long end_idx = slen - 1;
    while (end_idx >= 0 && (data[end_idx] == ' ' || data[end_idx] == '\t' || data[end_idx] == '\n' || data[end_idx] == '\r')) {
        end_idx--;
    }
    long new_len = end_idx + 1;
    if (new_len < 0) new_len = 0;
    void *obj = gc_alloc(8 + new_len + 1, GC_TAG_STRING, 0);
    *(long *)obj = new_len;
    if (new_len > 0) memcpy((char *)obj + 8, data, new_len);
    ((char *)obj + 8)[new_len] = '\0';
    return obj;
}

long __pluto_string_last_index_of(void *haystack, void *needle) {
    long hlen = *(long *)haystack;
    long nlen = *(long *)needle;
    if (nlen == 0) return hlen;
    if (nlen > hlen) return -1;

    const char *hdata = (const char *)haystack + 8;
    const char *ndata = (const char *)needle + 8;

    for (long i = hlen - nlen; i >= 0; i--) {
        if (memcmp(hdata + i, ndata, nlen) == 0) {
            return i;
        }
    }
    return -1;
}

long __pluto_string_count(void *haystack, void *needle) {
    long hlen = *(long *)haystack;
    long nlen = *(long *)needle;
    if (nlen == 0) return 0;
    if (nlen > hlen) return 0;

    const char *hdata = (const char *)haystack + 8;
    const char *ndata = (const char *)needle + 8;

    long count = 0;
    for (long i = 0; i <= hlen - nlen; i++) {
        if (memcmp(hdata + i, ndata, nlen) == 0) {
            count++;
            i += nlen - 1;
        }
    }
    return count;
}

long __pluto_string_is_empty(void *s) {
    long slen = *(long *)s;
    return slen == 0 ? 1 : 0;
}

long __pluto_string_is_whitespace(void *s) {
    long slen = *(long *)s;
    const char *data = (const char *)s + 8;
    if (slen == 0) return 1;
    for (long i = 0; i < slen; i++) {
        if (data[i] != ' ' && data[i] != '\t' && data[i] != '\n' && data[i] != '\r') {
            return 0;
        }
    }
    return 1;
}

void *__pluto_string_repeat(void *s, long count) {
    long slen = *(long *)s;
    const char *data = (const char *)s + 8;
    if (count <= 0) {
        void *obj = gc_alloc(8 + 1, GC_TAG_STRING, 0);
        *(long *)obj = 0;
        ((char *)obj + 8)[0] = '\0';
        return obj;
    }

    long new_len = slen * count;
    void *obj = gc_alloc(8 + new_len + 1, GC_TAG_STRING, 0);
    *(long *)obj = new_len;
    char *result = (char *)obj + 8;
    for (long i = 0; i < count; i++) {
        memcpy(result + i * slen, data, slen);
    }
    result[new_len] = '\0';
    return obj;
}

long __pluto_json_parse_int(void *s) {
    long slen = *(long *)s;
    const char *data = (const char *)s + 8;
    char *tmp = (char *)malloc(slen + 1);
    memcpy(tmp, data, slen);
    tmp[slen] = '\0';
    long result = strtol(tmp, NULL, 10);
    free(tmp);
    return result;
}

double __pluto_json_parse_float(void *s) {
    long slen = *(long *)s;
    const char *data = (const char *)s + 8;
    char *tmp = (char *)malloc(slen + 1);
    memcpy(tmp, data, slen);
    tmp[slen] = '\0';
    double result = strtod(tmp, NULL);
    free(tmp);
    return result;
}

void *__pluto_codepoint_to_string(long cp) {
    char buf[4];
    int len = 0;
    if (cp < 0x80) {
        buf[0] = (char)cp;
        len = 1;
    } else if (cp < 0x800) {
        buf[0] = (char)(0xC0 | (cp >> 6));
        buf[1] = (char)(0x80 | (cp & 0x3F));
        len = 2;
    } else {
        buf[0] = (char)(0xE0 | (cp >> 12));
        buf[1] = (char)(0x80 | ((cp >> 6) & 0x3F));
        buf[2] = (char)(0x80 | (cp & 0x3F));
        len = 3;
    }
    void *header = gc_alloc(8 + len + 1, GC_TAG_STRING, 0);
    *(long *)header = len;
    memcpy((char *)header + 8, buf, len);
    ((char *)header + 8)[len] = '\0';
    return header;
}

void *__pluto_int_to_string(long value) {
    int len = snprintf(NULL, 0, "%ld", value);
    size_t alloc_size = 8 + len + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = len;
    snprintf((char *)header + 8, len + 1, "%ld", value);
    return header;
}

void *__pluto_float_to_string(double value) {
    int len = snprintf(NULL, 0, "%f", value);
    size_t alloc_size = 8 + len + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = len;
    snprintf((char *)header + 8, len + 1, "%f", value);
    return header;
}

void *__pluto_bool_to_string(int value) {
    const char *s = value ? "true" : "false";
    long len = value ? 4 : 5;
    size_t alloc_size = 8 + len + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = len;
    memcpy((char *)header + 8, s, len);
    ((char *)header)[8 + len] = '\0';
    return header;
}

// ── Error handling runtime ────────────────────────────────────────────────────

void __pluto_raise_error(void *error_obj) {
    __pluto_current_error = error_obj;
}

long __pluto_has_error() {
    return __pluto_current_error != NULL ? 1 : 0;
}

void *__pluto_get_error() {
    return __pluto_current_error;
}

void __pluto_clear_error() {
    __pluto_current_error = NULL;
}

// Time
long __pluto_time_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (long)ts.tv_sec * 1000000000L + (long)ts.tv_nsec;
}

long __pluto_time_wall_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    return (long)ts.tv_sec * 1000000000L + (long)ts.tv_nsec;
}

void __pluto_time_sleep_ns(long ns) {
    struct timespec req;
    req.tv_sec = ns / 1000000000L;
    req.tv_nsec = ns % 1000000000L;
    nanosleep(&req, NULL);
}

// Random — xorshift64*
static unsigned long long __pluto_rng_state = 0;
static int __pluto_rng_seeded = 0;

static void __pluto_rng_ensure_seeded(void) {
    if (!__pluto_rng_seeded) {
        struct timespec ts;
        clock_gettime(CLOCK_MONOTONIC, &ts);
        __pluto_rng_state = (unsigned long long)ts.tv_sec * 1000000000ULL + (unsigned long long)ts.tv_nsec;
        if (__pluto_rng_state == 0) __pluto_rng_state = 1;
        __pluto_rng_seeded = 1;
    }
}

void __pluto_random_seed(long seed) {
    __pluto_rng_state = (unsigned long long)seed;
    if (__pluto_rng_state == 0) __pluto_rng_state = 1;
    __pluto_rng_seeded = 1;
}

long __pluto_random_int(void) {
    __pluto_rng_ensure_seeded();
    __pluto_rng_state ^= __pluto_rng_state >> 12;
    __pluto_rng_state ^= __pluto_rng_state << 25;
    __pluto_rng_state ^= __pluto_rng_state >> 27;
    return (long)(__pluto_rng_state * 0x2545F4914F6CDD1DULL);
}

double __pluto_random_float(void) {
    long r = __pluto_random_int();
    unsigned long long u = (unsigned long long)r;
    return (double)(u >> 11) * (1.0 / (double)(1ULL << 53));
}

// GC introspection
long __pluto_gc_heap_size(void) {
    return (long)gc_bytes_allocated;
}

// ── Socket runtime — POSIX sockets for networking ─────────────────────────────

__attribute__((constructor))
static void __pluto_ignore_sigpipe(void) {
    signal(SIGPIPE, SIG_IGN);
}

long __pluto_socket_create(long domain, long type, long protocol) {
    return (long)socket((int)domain, (int)type, (int)protocol);
}

long __pluto_socket_bind(long fd, void *host_str, long port) {
    const char *host = (const char *)host_str + 8;
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_port = htons((uint16_t)port);
    if (inet_pton(AF_INET, host, &addr.sin_addr) != 1) return -1;
    return bind((int)fd, (struct sockaddr *)&addr, sizeof(addr)) == 0 ? 0 : -1;
}

long __pluto_socket_listen(long fd, long backlog) {
    return listen((int)fd, (int)backlog) == 0 ? 0 : -1;
}

long __pluto_socket_accept(long fd) {
    struct sockaddr_in client_addr;
    socklen_t client_len = sizeof(client_addr);
    return (long)accept((int)fd, (struct sockaddr *)&client_addr, &client_len);
}

long __pluto_socket_connect(long fd, void *host_str, long port) {
    const char *host = (const char *)host_str + 8;
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_port = htons((uint16_t)port);
    if (inet_pton(AF_INET, host, &addr.sin_addr) != 1) return -1;
    return connect((int)fd, (struct sockaddr *)&addr, sizeof(addr)) == 0 ? 0 : -1;
}

void *__pluto_socket_read(long fd, long max_bytes) {
    if (max_bytes <= 0) {
        return __pluto_string_new("", 0);
    }
    if (max_bytes > 1048576) max_bytes = 1048576;
    char *buf = (char *)malloc(max_bytes);
    if (!buf) return __pluto_string_new("", 0);
    ssize_t n = read((int)fd, buf, (size_t)max_bytes);
    if (n <= 0) {
        free(buf);
        return __pluto_string_new("", 0);
    }
    void *result = __pluto_string_new(buf, n);
    free(buf);
    return result;
}

long __pluto_socket_write(long fd, void *data_str) {
    long len = *(long *)data_str;
    const char *data = (const char *)data_str + 8;
    return (long)write((int)fd, data, (size_t)len);
}

long __pluto_socket_close(long fd) {
    return close((int)fd) == 0 ? 0 : -1;
}

long __pluto_socket_set_reuseaddr(long fd) {
    int opt = 1;
    return setsockopt((int)fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt)) == 0 ? 0 : -1;
}

long __pluto_socket_get_port(long fd) {
    struct sockaddr_in addr;
    socklen_t len = sizeof(addr);
    if (getsockname((int)fd, (struct sockaddr *)&addr, &len) != 0) return -1;
    return (long)ntohs(addr.sin_port);
}

// ── Map and Set runtime ───────────────────────────────────────────────────────
// Key type tags: 0=int, 1=float, 2=bool, 3=string, 4=enum (discriminant)
// Open addressing with linear probing.  Meta byte: 0=empty, 0x80=occupied.

#define MAP_INIT_CAP 8
#define MAP_LOAD_FACTOR_NUM 3
#define MAP_LOAD_FACTOR_DEN 4

static unsigned long ht_hash(long key, long key_type) {
    unsigned long h;
    switch (key_type) {
    case 1: { // float — bitcast
        double d;
        memcpy(&d, &key, sizeof(double));
        unsigned long bits;
        memcpy(&bits, &d, sizeof(unsigned long));
        h = bits * 0x9e3779b97f4a7c15ULL;
        break;
    }
    case 3: { // string — FNV-1a
        void *s = (void *)key;
        long slen = *(long *)s;
        const unsigned char *data = (const unsigned char *)s + 8;
        h = 0xcbf29ce484222325ULL;
        for (long i = 0; i < slen; i++) {
            h ^= data[i];
            h *= 0x100000001b3ULL;
        }
        break;
    }
    default: // int(0), bool(2), enum(4)
        h = (unsigned long)key * 0x9e3779b97f4a7c15ULL;
        break;
    }
    return h;
}

static int ht_eq(long a, long b, long key_type) {
    if (key_type == 3) return __pluto_string_eq((void *)a, (void *)b);
    return a == b;
}

// ── Map API ──────────────────────────────────────────────────────────────────
// Handle layout (40 bytes, 5 fields): [count][capacity][keys_ptr][vals_ptr][meta_ptr]

static void map_grow(long *handle, long key_type);

void *__pluto_map_new(long key_type) {
    long *h = (long *)gc_alloc(40, GC_TAG_MAP, 5);
    h[0] = 0;            // count
    h[1] = MAP_INIT_CAP; // capacity
    h[2] = (long)calloc(MAP_INIT_CAP, 8);        // keys
    h[3] = (long)calloc(MAP_INIT_CAP, 8);        // vals
    h[4] = (long)calloc(MAP_INIT_CAP, 1);        // meta
    (void)key_type;
    return h;
}

void __pluto_map_insert(void *handle, long key_type, long key, long value) {
    long *h = (long *)handle;
    long count = h[0], cap = h[1];
    // Grow if load > 75%
    if (count * MAP_LOAD_FACTOR_DEN >= cap * MAP_LOAD_FACTOR_NUM) {
        map_grow(h, key_type);
        cap = h[1];
    }
    long *keys = (long *)h[2]; long *vals = (long *)h[3];
    unsigned char *meta = (unsigned char *)h[4];
    unsigned long idx = ht_hash(key, key_type) & (unsigned long)(cap - 1);
    while (1) {
        if (meta[idx] == 0) { // empty
            keys[idx] = key; vals[idx] = value; meta[idx] = 0x80;
            h[0] = count + 1;
            return;
        }
        if (meta[idx] >= 0x80 && ht_eq(keys[idx], key, key_type)) { // overwrite
            vals[idx] = value;
            return;
        }
        idx = (idx + 1) & (unsigned long)(cap - 1);
    }
}

long __pluto_map_get(void *handle, long key_type, long key) {
    long *h = (long *)handle;
    long cap = h[1];
    long *keys = (long *)h[2]; long *vals = (long *)h[3];
    unsigned char *meta = (unsigned char *)h[4];
    unsigned long idx = ht_hash(key, key_type) & (unsigned long)(cap - 1);
    while (1) {
        if (meta[idx] == 0) {
            fprintf(stderr, "pluto: map key not found\n");
            exit(1);
        }
        if (meta[idx] >= 0x80 && ht_eq(keys[idx], key, key_type)) {
            return vals[idx];
        }
        idx = (idx + 1) & (unsigned long)(cap - 1);
    }
}

long __pluto_map_contains(void *handle, long key_type, long key) {
    long *h = (long *)handle;
    long cap = h[1];
    long *keys = (long *)h[2];
    unsigned char *meta = (unsigned char *)h[4];
    unsigned long idx = ht_hash(key, key_type) & (unsigned long)(cap - 1);
    while (1) {
        if (meta[idx] == 0) return 0;
        if (meta[idx] >= 0x80 && ht_eq(keys[idx], key, key_type)) return 1;
        idx = (idx + 1) & (unsigned long)(cap - 1);
    }
}

void __pluto_map_remove(void *handle, long key_type, long key) {
    long *h = (long *)handle;
    long cap = h[1];
    long *keys = (long *)h[2];
    unsigned char *meta = (unsigned char *)h[4];
    unsigned long idx = ht_hash(key, key_type) & (unsigned long)(cap - 1);
    while (1) {
        if (meta[idx] == 0) return; // not found
        if (meta[idx] >= 0x80 && ht_eq(keys[idx], key, key_type)) {
            // Robin Hood / backward-shift deletion for correctness with linear probing
            unsigned long empty = idx;
            meta[empty] = 0;
            unsigned long j = (empty + 1) & (unsigned long)(cap - 1);
            while (meta[j] >= 0x80) {
                unsigned long natural = ht_hash(keys[j], key_type) & (unsigned long)(cap - 1);
                // Check if j is displaced past empty (wrapping)
                int displaced;
                if (empty <= j) displaced = (natural <= empty || natural > j);
                else             displaced = (natural <= empty && natural > j);
                if (displaced) {
                    keys[empty] = keys[j];
                    ((long *)h[3])[empty] = ((long *)h[3])[j];
                    meta[empty] = meta[j];
                    meta[j] = 0;
                    empty = j;
                }
                j = (j + 1) & (unsigned long)(cap - 1);
            }
            h[0]--;
            return;
        }
        idx = (idx + 1) & (unsigned long)(cap - 1);
    }
}

long __pluto_map_len(void *handle) {
    return ((long *)handle)[0];
}

void *__pluto_map_keys(void *handle) {
    long *h = (long *)handle;
    long cap = h[1];
    long *keys = (long *)h[2];
    unsigned char *meta = (unsigned char *)h[4];
    void *arr = __pluto_array_new(h[0] > 0 ? h[0] : 4);
    for (long i = 0; i < cap; i++) {
        if (meta[i] >= 0x80) __pluto_array_push(arr, keys[i]);
    }
    return arr;
}

void *__pluto_map_values(void *handle) {
    long *h = (long *)handle;
    long cap = h[1];
    long *vals = (long *)h[3];
    unsigned char *meta = (unsigned char *)h[4];
    void *arr = __pluto_array_new(h[0] > 0 ? h[0] : 4);
    for (long i = 0; i < cap; i++) {
        if (meta[i] >= 0x80) __pluto_array_push(arr, vals[i]);
    }
    return arr;
}

static void map_grow(long *h, long key_type) {
    long old_cap = h[1];
    if (old_cap > LONG_MAX / 2) {
        fprintf(stderr, "pluto: map capacity overflow\n");
        exit(1);
    }
    long new_cap = old_cap * 2;
    long *old_keys = (long *)h[2]; long *old_vals = (long *)h[3];
    unsigned char *old_meta = (unsigned char *)h[4];
    long *new_keys = (long *)calloc(new_cap, 8);
    long *new_vals = (long *)calloc(new_cap, 8);
    unsigned char *new_meta = (unsigned char *)calloc(new_cap, 1);
    for (long i = 0; i < old_cap; i++) {
        if (old_meta[i] >= 0x80) {
            unsigned long idx = ht_hash(old_keys[i], key_type) & (unsigned long)(new_cap - 1);
            while (new_meta[idx] >= 0x80) idx = (idx + 1) & (unsigned long)(new_cap - 1);
            new_keys[idx] = old_keys[i]; new_vals[idx] = old_vals[i]; new_meta[idx] = 0x80;
        }
    }
    free(old_keys); free(old_vals); free(old_meta);
    h[1] = new_cap; h[2] = (long)new_keys; h[3] = (long)new_vals; h[4] = (long)new_meta;
}

// ── Set API ──────────────────────────────────────────────────────────────────
// Handle layout (32 bytes, 4 fields): [count][capacity][keys_ptr][meta_ptr]

static void set_grow(long *h, long key_type);

void *__pluto_set_new(long key_type) {
    long *h = (long *)gc_alloc(32, GC_TAG_SET, 4);
    h[0] = 0;
    h[1] = MAP_INIT_CAP;
    h[2] = (long)calloc(MAP_INIT_CAP, 8);
    h[3] = (long)calloc(MAP_INIT_CAP, 1);
    (void)key_type;
    return h;
}

void __pluto_set_insert(void *handle, long key_type, long elem) {
    long *h = (long *)handle;
    long count = h[0], cap = h[1];
    if (count * MAP_LOAD_FACTOR_DEN >= cap * MAP_LOAD_FACTOR_NUM) {
        set_grow(h, key_type);
        cap = h[1];
    }
    long *keys = (long *)h[2];
    unsigned char *meta = (unsigned char *)h[3];
    unsigned long idx = ht_hash(elem, key_type) & (unsigned long)(cap - 1);
    while (1) {
        if (meta[idx] == 0) {
            keys[idx] = elem; meta[idx] = 0x80;
            h[0] = count + 1;
            return;
        }
        if (meta[idx] >= 0x80 && ht_eq(keys[idx], elem, key_type)) return; // already present
        idx = (idx + 1) & (unsigned long)(cap - 1);
    }
}

long __pluto_set_contains(void *handle, long key_type, long elem) {
    long *h = (long *)handle;
    long cap = h[1];
    long *keys = (long *)h[2];
    unsigned char *meta = (unsigned char *)h[3];
    unsigned long idx = ht_hash(elem, key_type) & (unsigned long)(cap - 1);
    while (1) {
        if (meta[idx] == 0) return 0;
        if (meta[idx] >= 0x80 && ht_eq(keys[idx], elem, key_type)) return 1;
        idx = (idx + 1) & (unsigned long)(cap - 1);
    }
}

void __pluto_set_remove(void *handle, long key_type, long elem) {
    long *h = (long *)handle;
    long cap = h[1];
    long *keys = (long *)h[2];
    unsigned char *meta = (unsigned char *)h[3];
    unsigned long idx = ht_hash(elem, key_type) & (unsigned long)(cap - 1);
    while (1) {
        if (meta[idx] == 0) return;
        if (meta[idx] >= 0x80 && ht_eq(keys[idx], elem, key_type)) {
            unsigned long empty = idx;
            meta[empty] = 0;
            unsigned long j = (empty + 1) & (unsigned long)(cap - 1);
            while (meta[j] >= 0x80) {
                unsigned long natural = ht_hash(keys[j], key_type) & (unsigned long)(cap - 1);
                int displaced;
                if (empty <= j) displaced = (natural <= empty || natural > j);
                else             displaced = (natural <= empty && natural > j);
                if (displaced) {
                    keys[empty] = keys[j]; meta[empty] = meta[j]; meta[j] = 0; empty = j;
                }
                j = (j + 1) & (unsigned long)(cap - 1);
            }
            h[0]--;
            return;
        }
        idx = (idx + 1) & (unsigned long)(cap - 1);
    }
}

long __pluto_set_len(void *handle) {
    return ((long *)handle)[0];
}

void *__pluto_set_to_array(void *handle) {
    long *h = (long *)handle;
    long cap = h[1];
    long *keys = (long *)h[2];
    unsigned char *meta = (unsigned char *)h[3];
    void *arr = __pluto_array_new(h[0] > 0 ? h[0] : 4);
    for (long i = 0; i < cap; i++) {
        if (meta[i] >= 0x80) __pluto_array_push(arr, keys[i]);
    }
    return arr;
}

static void set_grow(long *h, long key_type) {
    long old_cap = h[1];
    if (old_cap > LONG_MAX / 2) {
        fprintf(stderr, "pluto: set capacity overflow\n");
        exit(1);
    }
    long new_cap = old_cap * 2;
    long *old_keys = (long *)h[2];
    unsigned char *old_meta = (unsigned char *)h[3];
    long *new_keys = (long *)calloc(new_cap, 8);
    unsigned char *new_meta = (unsigned char *)calloc(new_cap, 1);
    for (long i = 0; i < old_cap; i++) {
        if (old_meta[i] >= 0x80) {
            unsigned long idx = ht_hash(old_keys[i], key_type) & (unsigned long)(new_cap - 1);
            while (new_meta[idx] >= 0x80) idx = (idx + 1) & (unsigned long)(new_cap - 1);
            new_keys[idx] = old_keys[i]; new_meta[idx] = 0x80;
        }
    }
    free(old_keys); free(old_meta);
    h[1] = new_cap; h[2] = (long)new_keys; h[3] = (long)new_meta;
}
// ── File I/O runtime ──────────────────────────────────────────────────────────

void *__pluto_fs_strerror(void) {
    const char *msg = strerror(errno);
    long len = (long)strlen(msg);
    return __pluto_string_new(msg, len);
}

long __pluto_fs_open_read(void *path_str) {
    const char *path = (const char *)path_str + 8;
    return (long)open(path, O_RDONLY);
}

long __pluto_fs_open_write(void *path_str) {
    const char *path = (const char *)path_str + 8;
    return (long)open(path, O_WRONLY | O_CREAT | O_TRUNC, 0644);
}

long __pluto_fs_open_append(void *path_str) {
    const char *path = (const char *)path_str + 8;
    return (long)open(path, O_WRONLY | O_CREAT | O_APPEND, 0644);
}

long __pluto_fs_close(long fd) {
    return close((int)fd) == 0 ? 0 : -1;
}

void *__pluto_fs_read(long fd, long max_bytes) {
    if (max_bytes <= 0) return __pluto_string_new("", 0);
    if (max_bytes > 104857600) max_bytes = 104857600; // 100MB cap
    char *buf = (char *)malloc((size_t)max_bytes);
    if (!buf) return __pluto_string_new("", 0);
    ssize_t n = read((int)fd, buf, (size_t)max_bytes);
    if (n <= 0) {
        free(buf);
        return __pluto_string_new("", 0);
    }
    void *result = __pluto_string_new(buf, n);
    free(buf);
    return result;
}

long __pluto_fs_write(long fd, void *data_str) {
    long len = *(long *)data_str;
    const char *data = (const char *)data_str + 8;
    ssize_t written = write((int)fd, data, (size_t)len);
    return (long)written;
}

long __pluto_fs_seek(long fd, long offset, long whence) {
    off_t result = lseek((int)fd, (off_t)offset, (int)whence);
    return (long)result;
}

void *__pluto_fs_read_all(void *path_str) {
    const char *path = (const char *)path_str + 8;
    int fd = open(path, O_RDONLY);
    if (fd < 0) return __pluto_string_new("", 0);
    struct stat st;
    if (fstat(fd, &st) != 0) {
        close(fd);
        return __pluto_string_new("", 0);
    }
    size_t size = (size_t)st.st_size;
    char *buf = (char *)malloc(size);
    if (!buf) {
        close(fd);
        return __pluto_string_new("", 0);
    }
    size_t total_read = 0;
    while (total_read < size) {
        ssize_t n = read(fd, buf + total_read, size - total_read);
        if (n <= 0) break;
        total_read += (size_t)n;
    }
    close(fd);
    void *result = __pluto_string_new(buf, (long)total_read);
    free(buf);
    return result;
}

long __pluto_fs_write_all(void *path_str, void *data_str) {
    const char *path = (const char *)path_str + 8;
    long len = *(long *)data_str;
    const char *data = (const char *)data_str + 8;
    int fd = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (fd < 0) return -1;
    size_t total_written = 0;
    while (total_written < (size_t)len) {
        ssize_t n = write(fd, data + total_written, (size_t)len - total_written);
        if (n <= 0) { close(fd); return -1; }
        total_written += (size_t)n;
    }
    close(fd);
    return 0;
}

long __pluto_fs_append_all(void *path_str, void *data_str) {
    const char *path = (const char *)path_str + 8;
    long len = *(long *)data_str;
    const char *data = (const char *)data_str + 8;
    int fd = open(path, O_WRONLY | O_CREAT | O_APPEND, 0644);
    if (fd < 0) return -1;
    size_t total_written = 0;
    while (total_written < (size_t)len) {
        ssize_t n = write(fd, data + total_written, (size_t)len - total_written);
        if (n <= 0) { close(fd); return -1; }
        total_written += (size_t)n;
    }
    close(fd);
    return 0;
}

long __pluto_fs_exists(void *path_str) {
    const char *path = (const char *)path_str + 8;
    struct stat st;
    return stat(path, &st) == 0 ? 1 : 0;
}

long __pluto_fs_file_size(void *path_str) {
    const char *path = (const char *)path_str + 8;
    struct stat st;
    if (stat(path, &st) != 0) return -1;
    return (long)st.st_size;
}

long __pluto_fs_is_dir(void *path_str) {
    const char *path = (const char *)path_str + 8;
    struct stat st;
    if (stat(path, &st) != 0) return 0;
    return S_ISDIR(st.st_mode) ? 1 : 0;
}

long __pluto_fs_is_file(void *path_str) {
    const char *path = (const char *)path_str + 8;
    struct stat st;
    if (stat(path, &st) != 0) return 0;
    return S_ISREG(st.st_mode) ? 1 : 0;
}

long __pluto_fs_remove(void *path_str) {
    const char *path = (const char *)path_str + 8;
    return unlink(path) == 0 ? 0 : -1;
}

long __pluto_fs_mkdir(void *path_str) {
    const char *path = (const char *)path_str + 8;
    return mkdir(path, 0755) == 0 ? 0 : -1;
}

long __pluto_fs_rmdir(void *path_str) {
    const char *path = (const char *)path_str + 8;
    return rmdir(path) == 0 ? 0 : -1;
}

long __pluto_fs_rename(void *from_str, void *to_str) {
    const char *from = (const char *)from_str + 8;
    const char *to = (const char *)to_str + 8;
    return rename(from, to) == 0 ? 0 : -1;
}

long __pluto_fs_copy(void *from_str, void *to_str) {
    const char *from = (const char *)from_str + 8;
    const char *to = (const char *)to_str + 8;
    int src_fd = open(from, O_RDONLY);
    if (src_fd < 0) return -1;
    int dst_fd = open(to, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (dst_fd < 0) { close(src_fd); return -1; }
    char buf[4096];
    ssize_t n;
    while ((n = read(src_fd, buf, sizeof(buf))) > 0) {
        size_t written = 0;
        while (written < (size_t)n) {
            ssize_t w = write(dst_fd, buf + written, (size_t)n - written);
            if (w <= 0) { close(src_fd); close(dst_fd); return -1; }
            written += (size_t)w;
        }
    }
    close(src_fd);
    close(dst_fd);
    return n < 0 ? -1 : 0;
}

void *__pluto_fs_list_dir(void *path_str) {
    const char *path = (const char *)path_str + 8;
    void *arr = __pluto_array_new(8);
    DIR *d = opendir(path);
    if (!d) return arr;
    struct dirent *entry;
    while ((entry = readdir(d)) != NULL) {
        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0)
            continue;
        long name_len = (long)strlen(entry->d_name);
        void *name_str = __pluto_string_new(entry->d_name, name_len);
        __pluto_array_push(arr, (long)name_str);
    }
    closedir(d);
    return arr;
}

void *__pluto_fs_temp_dir(void) {
    char tmpl[] = "/tmp/pluto_XXXXXX";
    char *result = mkdtemp(tmpl);
    if (!result) return __pluto_string_new("", 0);
    long len = (long)strlen(result);
    return __pluto_string_new(result, len);
}

long __pluto_fs_seek_set(void) { return (long)SEEK_SET; }
long __pluto_fs_seek_cur(void) { return (long)SEEK_CUR; }
long __pluto_fs_seek_end(void) { return (long)SEEK_END; }

// ── Math builtins ─────────────────────────────────────────────────────────────

long __pluto_abs_int(long x) {
    return x < 0 ? -x : x;
}

double __pluto_abs_float(double x) {
    return fabs(x);
}

long __pluto_min_int(long a, long b) {
    return a < b ? a : b;
}

double __pluto_min_float(double a, double b) {
    return a < b ? a : b;
}

long __pluto_max_int(long a, long b) {
    return a > b ? a : b;
}

double __pluto_max_float(double a, double b) {
    return a > b ? a : b;
}

long __pluto_pow_int(long base, long exp) {
    if (exp < 0) {
        // Raise MathError via the runtime error system
        const char *msg = "negative exponent in integer pow";
        void *msg_str = __pluto_string_new(msg, (long)strlen(msg));
        void *err_obj = __pluto_alloc(8); // 1 field: message
        *(long *)err_obj = (long)msg_str;
        __pluto_raise_error(err_obj);
        return 0;
    }
    long result = 1;
    long b = base;
    long e = exp;
    while (e > 0) {
        if (e & 1) result *= b;
        b *= b;
        e >>= 1;
    }
    return result;
}

double __pluto_pow_float(double base, double exp) {
    return pow(base, exp);
}

double __pluto_sqrt(double x) {
    return sqrt(x);
}

double __pluto_floor(double x) {
    return floor(x);
}

double __pluto_ceil(double x) {
    return ceil(x);
}

double __pluto_round(double x) {
    return round(x);
}

double __pluto_sin(double x) {
    return sin(x);
}

double __pluto_cos(double x) {
    return cos(x);
}

double __pluto_tan(double x) {
    return tan(x);
}

double __pluto_log(double x) {
    return log(x);
}

// ── Test framework ────────────────────────────────────────────────────────────

void __pluto_expect_equal_int(long actual, long expected, long line) {
    if (actual != expected) {
        fprintf(stderr, "FAIL (line %ld): expected %ld to equal %ld\n", line, actual, expected);
        exit(1);
    }
}

void __pluto_expect_equal_float(double actual, double expected, long line) {
    if (actual != expected) {
        fprintf(stderr, "FAIL (line %ld): expected %f to equal %f\n", line, actual, expected);
        exit(1);
    }
}

void __pluto_expect_equal_bool(long actual, long expected, long line) {
    const char *a_str = actual ? "true" : "false";
    const char *e_str = expected ? "true" : "false";
    if (actual != expected) {
        fprintf(stderr, "FAIL (line %ld): expected %s to equal %s\n", line, a_str, e_str);
        exit(1);
    }
}

void __pluto_expect_equal_string(void *actual, void *expected, long line) {
    if (!__pluto_string_eq(actual, expected)) {
        long len_a = *(long *)actual;
        long len_e = *(long *)expected;
        const char *data_a = (const char *)actual + 8;
        const char *data_e = (const char *)expected + 8;
        fprintf(stderr, "FAIL (line %ld): expected \"%.*s\" to equal \"%.*s\"\n",
                line, (int)len_a, data_a, (int)len_e, data_e);
        exit(1);
    }
}

void __pluto_expect_true(long actual, long line) {
    if (!actual) {
        fprintf(stderr, "FAIL (line %ld): expected true but got false\n", line);
        exit(1);
    }
}

void __pluto_expect_false(long actual, long line) {
    if (actual) {
        fprintf(stderr, "FAIL (line %ld): expected false but got true\n", line);
        exit(1);
    }
}

void __pluto_test_start(void *name_str) {
    long len = *(long *)name_str;
    const char *data = (const char *)name_str + 8;
    printf("test %.*s ... ", (int)len, data);
    fflush(stdout);
}

void __pluto_test_pass(void) {
    printf("ok\n");
}

void __pluto_test_summary(long count) {
    printf("\n%ld tests passed\n", count);
}


// ── HTTP runtime ──────────────────────────────────────────────────────────────

void *__pluto_http_read_request(long fd) {
    // Read from socket until we have complete HTTP headers (double CRLF)
    // Then read Content-Length bytes for the body
    int buf_cap = 4096;
    char *buf = (char *)malloc(buf_cap);
    int buf_len = 0;
    int headers_end = -1;

    while (1) {
        if (buf_len + 1024 > buf_cap) {
            buf_cap *= 2;
            buf = (char *)realloc(buf, buf_cap);
        }
        ssize_t n = read((int)fd, buf + buf_len, 1024);
        if (n <= 0) {
            free(buf);
            return __pluto_string_new("", 0);
        }
        buf_len += (int)n;

        // Search for \r\n\r\n
        for (int i = headers_end < 0 ? 0 : headers_end; i <= buf_len - 4; i++) {
            if (buf[i] == '\r' && buf[i+1] == '\n' && buf[i+2] == '\r' && buf[i+3] == '\n') {
                headers_end = i + 4;
                break;
            }
        }
        if (headers_end < 0) continue;

        // Look for Content-Length in headers
        long content_length = 0;
        {
            const char *cl = "Content-Length:";
            int cl_len = 15;
            for (int i = 0; i < headers_end - cl_len; i++) {
                if (strncasecmp(buf + i, cl, cl_len) == 0) {
                    content_length = strtol(buf + i + cl_len, NULL, 10);
                    break;
                }
            }
        }

        int total_needed = headers_end + (int)content_length;
        // Read remaining body bytes if needed
        while (buf_len < total_needed) {
            if (buf_len + 1024 > buf_cap) {
                buf_cap *= 2;
                buf = (char *)realloc(buf, buf_cap);
            }
            ssize_t n2 = read((int)fd, buf + buf_len, (size_t)(total_needed - buf_len));
            if (n2 <= 0) break;
            buf_len += (int)n2;
        }

        void *result = __pluto_string_new(buf, buf_len);
        free(buf);
        return result;
    }
}

void *__pluto_http_url_decode(void *pluto_str) {
    long slen = *(long *)pluto_str;
    const char *src = (const char *)pluto_str + 8;
    char *out = (char *)malloc(slen + 1);
    int olen = 0;
    for (long i = 0; i < slen; i++) {
        if (src[i] == '%' && i + 2 < slen) {
            char h1 = src[i+1], h2 = src[i+2];
            int v = 0;
            if (h1 >= '0' && h1 <= '9') v = (h1 - '0') << 4;
            else if (h1 >= 'a' && h1 <= 'f') v = (10 + h1 - 'a') << 4;
            else if (h1 >= 'A' && h1 <= 'F') v = (10 + h1 - 'A') << 4;
            if (h2 >= '0' && h2 <= '9') v |= h2 - '0';
            else if (h2 >= 'a' && h2 <= 'f') v |= 10 + h2 - 'a';
            else if (h2 >= 'A' && h2 <= 'F') v |= 10 + h2 - 'A';
            out[olen++] = (char)v;
            i += 2;
        } else if (src[i] == '+') {
            out[olen++] = ' ';
        } else {
            out[olen++] = src[i];
        }
    }
    void *result = __pluto_string_new(out, olen);
    free(out);
    return result;
}

// ── Concurrency ─────────────────────────────────────────────────────────────

// Task handle layout (56 bytes, 7 slots):
//   [0] closure   (i64, GC pointer)
//   [1] result    (i64)
//   [2] error     (i64, GC pointer)
//   [3] done      (i64)
//   [4] sync_ptr  (i64, raw malloc — NULL in test mode)
//   [5] detached  (i64, 0 or 1)
//   [6] cancelled (i64, 0 or 1)

static void task_raise_cancelled(void) {
    const char *msg = "task cancelled";
    void *msg_str = __pluto_string_new((char *)msg, (long)strlen(msg));
    void *err_obj = __pluto_alloc(8);  // 1 field: message
    *(long *)err_obj = (long)msg_str;
    __pluto_raise_error(err_obj);
}

#ifdef PLUTO_TEST_MODE

// ── Fiber scheduler infrastructure ──────────────────────────────────────────

#define FIBER_STACK_SIZE (64 * 1024)   // 64KB per fiber stack
#define MAX_FIBERS 256

typedef enum { STRATEGY_SEQUENTIAL=0, STRATEGY_ROUND_ROBIN=1, STRATEGY_RANDOM=2, STRATEGY_EXHAUSTIVE=3 } Strategy;
typedef enum {
    FIBER_READY=0, FIBER_RUNNING=1,
    FIBER_BLOCKED_TASK=2, FIBER_BLOCKED_CHAN_SEND=3,
    FIBER_BLOCKED_CHAN_RECV=4, FIBER_BLOCKED_SELECT=5,
    FIBER_COMPLETED=6
} FiberState;

typedef struct {
    ucontext_t context;
    char *stack;
    FiberState state;
    long *task;              // associated task handle (NULL for fiber 0 / main test fiber)
    long closure_ptr;        // closure to execute (for spawned fibers)
    void *blocked_on;        // task handle or channel handle we're waiting on
    long blocked_value;      // value for pending send
    int id;
    // Per-fiber saved TLS state (restored on context switch)
    void *saved_error;       // __pluto_current_error
    long *saved_current_task; // __pluto_current_task
} Fiber;

typedef struct {
    Fiber fibers[MAX_FIBERS];
    int fiber_count;
    int current_fiber;
    Strategy strategy;
    uint64_t seed;
    long main_fn_ptr;        // test function pointer (fiber 0 entry)
    ucontext_t scheduler_ctx;
    int deadlock;
} Scheduler;

static Scheduler *g_scheduler = NULL;

// ── Exhaustive (DPOR) state ─────────────────────────────────────────────────

#define EXHST_MAX_DEPTH 200
#define EXHST_MAX_CHANNELS_PER_FIBER 32
#define EXHST_MAX_FAILURES 64

typedef struct {
    // Current schedule trace
    int choices[EXHST_MAX_DEPTH];                  // fiber index chosen at each yield point
    int ready[EXHST_MAX_DEPTH][MAX_FIBERS];        // ready fibers at each yield point
    int ready_count[EXHST_MAX_DEPTH];              // count of ready fibers at each yield
    int depth;                                      // current yield point index

    // Replay state for backtracking
    int replay_prefix[EXHST_MAX_DEPTH];            // choices to replay
    int replay_len;                                 // how many choices to replay
    int replay_next_choice;                         // forced choice after replay

    // DPOR: channel dependency tracking per fiber (per-schedule, reset each run)
    void *fiber_channels[MAX_FIBERS][EXHST_MAX_CHANNELS_PER_FIBER];
    int fiber_channel_count[MAX_FIBERS];

    // DPOR: accumulated dependency matrix (persistent across schedules)
    int dep_matrix[MAX_FIBERS][MAX_FIBERS];        // 1 = fibers share channels
    int dep_valid;                                  // 1 once first schedule observed

    // Bookkeeping
    int schedules_explored;
    int max_schedules;
    int max_depth;
    int fiber_count_snapshot;                       // fiber count for dep matrix update

    // Failure collection
    int failure_count;
    char *failure_messages[EXHST_MAX_FAILURES];
} ExhaustiveState;

static ExhaustiveState *g_exhaustive = NULL;

// Forward declarations for fiber scheduler
static void scheduler_run(void);
static void fiber_yield_to_scheduler(void);
static void test_main_fiber_entry(void);

// ── Fiber helper functions ──────────────────────────────────────────────────

static void wake_fibers_blocked_on_task(long *task_ptr) {
    if (!g_scheduler) return;
    for (int i = 0; i < g_scheduler->fiber_count; i++) {
        Fiber *f = &g_scheduler->fibers[i];
        if (f->state == FIBER_BLOCKED_TASK && f->blocked_on == (void *)task_ptr) {
            f->state = FIBER_READY;
            f->blocked_on = NULL;
        }
    }
}

static void wake_fibers_blocked_on_chan(long *ch_ptr) {
    if (!g_scheduler) return;
    for (int i = 0; i < g_scheduler->fiber_count; i++) {
        Fiber *f = &g_scheduler->fibers[i];
        if ((f->state == FIBER_BLOCKED_CHAN_SEND || f->state == FIBER_BLOCKED_CHAN_RECV ||
             f->state == FIBER_BLOCKED_SELECT) && f->blocked_on == (void *)ch_ptr) {
            f->state = FIBER_READY;
            f->blocked_on = NULL;
        }
    }
}

// Wake ALL fibers blocked on select that include this channel in their buffer
static void wake_select_fibers_for_chan(long *ch_ptr) {
    if (!g_scheduler) return;
    for (int i = 0; i < g_scheduler->fiber_count; i++) {
        Fiber *f = &g_scheduler->fibers[i];
        if (f->state == FIBER_BLOCKED_SELECT) {
            // For select, blocked_on points to the buffer_ptr array
            // We wake unconditionally since we can't cheaply check all handles
            f->state = FIBER_READY;
            f->blocked_on = NULL;
        }
    }
}

static uint64_t lcg_next(uint64_t *seed) {
    *seed = (*seed) * 6364136223846793005ULL + 1442695040888963407ULL;
    return *seed;
}

// ── Exhaustive helper functions ─────────────────────────────────────────────

static void exhaustive_record_channel(int fiber_id, void *channel) {
    if (!g_exhaustive) return;
    ExhaustiveState *es = g_exhaustive;
    if (fiber_id < 0 || fiber_id >= MAX_FIBERS) return;
    // Deduplicate
    for (int i = 0; i < es->fiber_channel_count[fiber_id]; i++) {
        if (es->fiber_channels[fiber_id][i] == channel) return;
    }
    if (es->fiber_channel_count[fiber_id] < EXHST_MAX_CHANNELS_PER_FIBER) {
        es->fiber_channels[fiber_id][es->fiber_channel_count[fiber_id]++] = channel;
    }
}

static void exhaustive_update_dep_matrix(ExhaustiveState *es, int fiber_count) {
    // After a complete schedule, update the dependency matrix.
    // Two fibers are dependent if they share at least one channel.
    for (int a = 0; a < fiber_count; a++) {
        for (int b = a + 1; b < fiber_count; b++) {
            int shared = 0;
            for (int ci = 0; ci < es->fiber_channel_count[a] && !shared; ci++) {
                for (int cj = 0; cj < es->fiber_channel_count[b] && !shared; cj++) {
                    if (es->fiber_channels[a][ci] == es->fiber_channels[b][cj])
                        shared = 1;
                }
            }
            if (shared) {
                es->dep_matrix[a][b] = 1;
                es->dep_matrix[b][a] = 1;
            }
        }
    }
    es->dep_valid = 1;
}

static int exhaustive_find_backtrack(ExhaustiveState *es) {
    // Walk backward through yield points to find an unexplored alternative.
    // With DPOR: skip alternatives that are independent of the chosen fiber.
    for (int i = es->depth - 1; i >= 0; i--) {
        int chosen = es->choices[i];
        int *rdy = es->ready[i];
        int rc = es->ready_count[i];

        if (rc <= 1) continue;  // only one choice at this yield point

        // Find position of chosen fiber in the ready set
        int pos = -1;
        for (int j = 0; j < rc; j++) {
            if (rdy[j] == chosen) { pos = j; break; }
        }
        if (pos < 0 || pos >= rc - 1) continue;  // no more alternatives

        // Try subsequent alternatives
        for (int j = pos + 1; j < rc; j++) {
            int alt = rdy[j];
            // DPOR pruning: skip if we know they're independent
            if (es->dep_valid && !es->dep_matrix[chosen][alt]) continue;

            // Found a viable backtrack point
            memcpy(es->replay_prefix, es->choices, i * sizeof(int));
            es->replay_len = i;
            es->replay_next_choice = alt;
            return 1;
        }
    }
    return 0;  // all schedules explored
}

static int pick_next_fiber(void) {
    if (!g_scheduler) return -1;
    int n = g_scheduler->fiber_count;

    if (g_scheduler->strategy == STRATEGY_ROUND_ROBIN) {
        // Round-robin: start from current+1, find first READY
        for (int off = 1; off <= n; off++) {
            int idx = (g_scheduler->current_fiber + off) % n;
            if (g_scheduler->fibers[idx].state == FIBER_READY) return idx;
        }
        return -1;
    } else if (g_scheduler->strategy == STRATEGY_EXHAUSTIVE && g_exhaustive) {
        // Exhaustive: DFS over schedule tree with DPOR pruning
        ExhaustiveState *es = g_exhaustive;

        // Collect ready fibers
        int ready[MAX_FIBERS];
        int ready_count = 0;
        for (int i = 0; i < n; i++) {
            if (g_scheduler->fibers[i].state == FIBER_READY) {
                ready[ready_count++] = i;
            }
        }
        if (ready_count == 0) return -1;

        if (es->depth >= es->max_depth) {
            // Past depth limit: pick first ready without recording
            return ready[0];
        }

        // Record the ready set at this yield point
        memcpy(es->ready[es->depth], ready, ready_count * sizeof(int));
        es->ready_count[es->depth] = ready_count;

        int choice;
        if (es->depth < es->replay_len) {
            // Replaying a prefix: use the predetermined choice
            choice = es->replay_prefix[es->depth];
        } else if (es->depth == es->replay_len && es->replay_next_choice >= 0) {
            // First new choice after replay: use the forced alternative
            choice = es->replay_next_choice;
            es->replay_next_choice = -1;
        } else {
            // New territory: pick the first ready fiber (DFS order)
            choice = ready[0];
        }

        es->choices[es->depth] = choice;
        es->depth++;
        return choice;
    } else {
        // Random: collect all READY fibers, pick one using LCG
        int ready[MAX_FIBERS];
        int ready_count = 0;
        for (int i = 0; i < n; i++) {
            if (g_scheduler->fibers[i].state == FIBER_READY) {
                ready[ready_count++] = i;
            }
        }
        if (ready_count == 0) return -1;
        uint64_t r = lcg_next(&g_scheduler->seed);
        return ready[(int)(r % (uint64_t)ready_count)];
    }
}

static int all_fibers_done(void) {
    for (int i = 0; i < g_scheduler->fiber_count; i++) {
        if (g_scheduler->fibers[i].state != FIBER_COMPLETED) return 0;
    }
    return 1;
}

static void fiber_yield_to_scheduler(void) {
    int cur = g_scheduler->current_fiber;
    Fiber *f = &g_scheduler->fibers[cur];
    // Save TLS state
    f->saved_error = __pluto_current_error;
    f->saved_current_task = __pluto_current_task;
    swapcontext(&f->context, &g_scheduler->scheduler_ctx);
    // Resumed — TLS state restored by scheduler before switching to us
}

static void fiber_entry_fn(int fiber_id) {
    Fiber *f = &g_scheduler->fibers[fiber_id];
    long *task = f->task;

    // Execute the closure
    long fn_ptr = *(long *)f->closure_ptr;
    long result = ((long(*)(long))fn_ptr)(f->closure_ptr);

    // Store result or error in task handle
    if (__pluto_current_error) {
        task[2] = (long)__pluto_current_error;
        __pluto_current_error = NULL;
    } else {
        task[1] = result;
    }
    task[3] = 1;  // done

    // If detached and errored, print to stderr
    if (task[5] && task[2]) {
        long *err_obj = (long *)task[2];
        char *msg_ptr = (char *)err_obj[0];
        if (msg_ptr) {
            long len = *(long *)msg_ptr;
            char *data = msg_ptr + 8;
            fprintf(stderr, "pluto: error in detached task: %.*s\n", (int)len, data);
        }
    }

    f->state = FIBER_COMPLETED;

    // Wake any fibers waiting on this task
    wake_fibers_blocked_on_task(task);

    // Return to scheduler via uc_link
}

static void test_main_fiber_entry(void) {
    // Execute the test function (no closure env, just a plain function pointer)
    ((void(*)(void))g_scheduler->main_fn_ptr)();
    g_scheduler->fibers[0].state = FIBER_COMPLETED;
    // Return to scheduler via uc_link
}

static void scheduler_run(void) {
    while (1) {
        int next = pick_next_fiber();
        if (next == -1) {
            if (all_fibers_done()) break;
            // Deadlock: all remaining fibers are blocked
            fprintf(stderr, "pluto: deadlock detected in test\n");
            for (int i = 0; i < g_scheduler->fiber_count; i++) {
                Fiber *f = &g_scheduler->fibers[i];
                if (f->state >= FIBER_BLOCKED_TASK && f->state <= FIBER_BLOCKED_SELECT) {
                    const char *reason = "unknown";
                    switch (f->state) {
                        case FIBER_BLOCKED_TASK:      reason = "task.get()"; break;
                        case FIBER_BLOCKED_CHAN_SEND:  reason = "chan.send()"; break;
                        case FIBER_BLOCKED_CHAN_RECV:  reason = "chan.recv()"; break;
                        case FIBER_BLOCKED_SELECT:     reason = "select"; break;
                        default: break;
                    }
                    fprintf(stderr, "  Fiber %d: blocked on %s\n", i, reason);
                }
            }
            g_scheduler->deadlock = 1;
            break;
        }

        // Restore next fiber's TLS state
        g_scheduler->current_fiber = next;
        gc_fiber_stacks.current_fiber = next;  // Tell GC which fiber is running
        Fiber *f = &g_scheduler->fibers[next];
        __pluto_current_error = f->saved_error;
        __pluto_current_task = f->saved_current_task;
        f->state = FIBER_RUNNING;

        swapcontext(&g_scheduler->scheduler_ctx, &f->context);

        gc_fiber_stacks.current_fiber = -1;  // Back in scheduler context

        // Fiber yielded or completed — state already saved in fiber_yield_to_scheduler
        // (or fiber completed and returned via uc_link)
        // For completed fibers that return via uc_link, save their state too
        Fiber *yielded = &g_scheduler->fibers[g_scheduler->current_fiber];
        if (yielded->state != FIBER_COMPLETED) {
            // State was saved by fiber_yield_to_scheduler already
        } else {
            yielded->saved_error = __pluto_current_error;
            yielded->saved_current_task = __pluto_current_task;
            gc_fiber_stacks.stacks[g_scheduler->current_fiber].active = 0;
        }
    }
}

// ── __pluto_test_run: entry point called by codegen ──

// Helper: create a fresh scheduler with fiber 0 and run it.
// Returns 1 if deadlock occurred, 0 otherwise.
static int test_run_single(long fn_ptr, Strategy strategy, uint64_t run_seed) {
    g_scheduler = (Scheduler *)calloc(1, sizeof(Scheduler));
    g_scheduler->strategy = strategy;
    g_scheduler->seed = run_seed;
    g_scheduler->main_fn_ptr = fn_ptr;

    // Create fiber 0 for the test body
    Fiber *f = &g_scheduler->fibers[0];
    f->id = 0;
    f->state = FIBER_READY;
    f->stack = (char *)malloc(FIBER_STACK_SIZE);
    f->task = NULL;
    f->closure_ptr = 0;
    f->saved_error = NULL;
    f->saved_current_task = NULL;
    getcontext(&f->context);
    f->context.uc_stack.ss_sp = f->stack;
    f->context.uc_stack.ss_size = FIBER_STACK_SIZE;
    f->context.uc_link = &g_scheduler->scheduler_ctx;
    makecontext(&f->context, (void(*)(void))test_main_fiber_entry, 0);
    g_scheduler->fiber_count = 1;

    // Register fiber 0 with GC fiber stack scanner
    memset(&gc_fiber_stacks, 0, sizeof(gc_fiber_stacks));
    gc_fiber_stacks.stacks[0].base = f->stack;
    gc_fiber_stacks.stacks[0].size = FIBER_STACK_SIZE;
    gc_fiber_stacks.stacks[0].active = 1;
    gc_fiber_stacks.count = 1;
    gc_fiber_stacks.current_fiber = -1;
    gc_fiber_stacks.enabled = 1;

    scheduler_run();

    gc_fiber_stacks.enabled = 0;
    int had_deadlock = g_scheduler->deadlock;
    int fiber_count = g_scheduler->fiber_count;

    for (int i = 0; i < fiber_count; i++)
        free(g_scheduler->fibers[i].stack);
    free(g_scheduler);
    g_scheduler = NULL;

    return had_deadlock;
}

void __pluto_test_run(long fn_ptr, long strategy, long seed, long iterations) {
    if (strategy == STRATEGY_SEQUENTIAL) {
        ((void(*)(void))fn_ptr)();
        return;
    }

    if (strategy == STRATEGY_EXHAUSTIVE) {
        // ── Exhaustive strategy: DFS over all interleavings with DPOR pruning ──
        int max_schedules = 10000;
        int max_depth = EXHST_MAX_DEPTH;
        char *env;
        env = getenv("PLUTO_MAX_SCHEDULES");
        if (env) max_schedules = (int)strtol(env, NULL, 0);
        env = getenv("PLUTO_MAX_DEPTH");
        if (env) {
            max_depth = (int)strtol(env, NULL, 0);
            if (max_depth > EXHST_MAX_DEPTH) max_depth = EXHST_MAX_DEPTH;
        }

        ExhaustiveState *es = (ExhaustiveState *)calloc(1, sizeof(ExhaustiveState));
        es->max_schedules = max_schedules;
        es->max_depth = max_depth;
        es->replay_len = 0;
        es->replay_next_choice = -1;  // first run: no forced choice, DFS picks first ready

        while (es->schedules_explored < es->max_schedules) {
            // Reset per-schedule state
            es->depth = 0;
            memset(es->fiber_channel_count, 0, sizeof(es->fiber_channel_count));
            g_exhaustive = es;

            int had_deadlock = test_run_single(fn_ptr, STRATEGY_EXHAUSTIVE, 0);

            es->fiber_count_snapshot = 0;  // infer from depth info
            g_exhaustive = NULL;

            // Collect failure info
            if (had_deadlock && es->failure_count < EXHST_MAX_FAILURES) {
                char msg[256];
                snprintf(msg, sizeof(msg), "deadlock in schedule %d (depth %d)",
                         es->schedules_explored, es->depth);
                es->failure_messages[es->failure_count++] = strdup(msg);
            }

            // Update DPOR dependency matrix from this schedule's channel accesses.
            // We need the fiber count — infer it from the scheduler that was just freed.
            // Since fibers are created incrementally (0..N-1), count from channel tracking.
            {
                int max_fiber = 0;
                for (int i = 0; i < MAX_FIBERS; i++) {
                    if (es->fiber_channel_count[i] > 0 && i + 1 > max_fiber)
                        max_fiber = i + 1;
                }
                // Also check the depth records for fibers that never touched channels
                for (int d = 0; d < es->depth; d++) {
                    for (int j = 0; j < es->ready_count[d]; j++) {
                        if (es->ready[d][j] + 1 > max_fiber)
                            max_fiber = es->ready[d][j] + 1;
                    }
                }
                if (max_fiber > 0)
                    exhaustive_update_dep_matrix(es, max_fiber);
            }

            es->schedules_explored++;

            // Find next unexplored schedule via backtracking
            if (!exhaustive_find_backtrack(es)) break;
        }

        // Report results
        fprintf(stderr, "  Exhaustive: %d schedule%s explored",
                es->schedules_explored, es->schedules_explored == 1 ? "" : "s");
        if (es->schedules_explored >= es->max_schedules) {
            fprintf(stderr, " (limit reached)");
        }
        fprintf(stderr, "\n");

        if (es->failure_count > 0) {
            fprintf(stderr, "  %d failure%s found:\n",
                    es->failure_count, es->failure_count == 1 ? "" : "s");
            for (int i = 0; i < es->failure_count; i++) {
                fprintf(stderr, "    - %s\n", es->failure_messages[i]);
                free(es->failure_messages[i]);
            }
            free(es);
            exit(1);
        }
        free(es);
        return;
    }

    // ── RoundRobin / Random strategies ──
    char *env_seed = getenv("PLUTO_TEST_SEED");
    if (env_seed) seed = (long)strtoull(env_seed, NULL, 0);
    char *env_iters = getenv("PLUTO_TEST_ITERATIONS");
    if (env_iters) iterations = (long)strtoull(env_iters, NULL, 0);

    int num_runs = (strategy == STRATEGY_RANDOM) ? (int)iterations : 1;
    if (num_runs < 1) num_runs = 1;

    for (int run = 0; run < num_runs; run++) {
        uint64_t run_seed = (uint64_t)seed + (uint64_t)run;
        int had_deadlock = test_run_single(fn_ptr, (Strategy)strategy, run_seed);
        if (had_deadlock) {
            fprintf(stderr, "  (seed: 0x%llx, iteration: %d)\n",
                    (unsigned long long)run_seed, run);
            exit(1);
        }
    }
}

// ── Test mode: task operations (fiber-aware) ────────────────────────────────

static long task_spawn_sequential(long closure_ptr) {
    // Phase A inline behavior (for sequential strategy or no scheduler)
    long *task = (long *)gc_alloc(56, GC_TAG_TASK, 3);
    task[0] = closure_ptr;
    task[1] = 0;  task[2] = 0;  task[3] = 0;
    task[4] = 0;  task[5] = 0;  task[6] = 0;

    long *prev_task = __pluto_current_task;
    void *prev_error = __pluto_current_error;
    __pluto_current_error = NULL;
    __pluto_current_task = task;

    long fn_ptr = *(long *)closure_ptr;
    long result = ((long(*)(long))fn_ptr)(closure_ptr);

    if (__pluto_current_error) {
        task[2] = (long)__pluto_current_error;
        __pluto_current_error = NULL;
    } else {
        task[1] = result;
    }
    task[3] = 1;

    if (task[5] && task[2]) {
        long *err_obj = (long *)task[2];
        char *msg_ptr = (char *)err_obj[0];
        if (msg_ptr) {
            long len = *(long *)msg_ptr;
            char *data = msg_ptr + 8;
            fprintf(stderr, "pluto: error in detached task: %.*s\n", (int)len, data);
        }
    }

    __pluto_current_task = prev_task;
    __pluto_current_error = prev_error;
    return (long)task;
}

static long task_spawn_fiber(long closure_ptr) {
    // Create a new fiber for the spawned task
    long *task = (long *)gc_alloc(56, GC_TAG_TASK, 3);
    task[0] = closure_ptr;
    task[1] = 0;  task[2] = 0;  task[3] = 0;
    task[4] = 0;  task[5] = 0;  task[6] = 0;

    int fid = g_scheduler->fiber_count;
    if (fid >= MAX_FIBERS) {
        fprintf(stderr, "pluto: too many fibers (max %d)\n", MAX_FIBERS);
        exit(1);
    }

    Fiber *f = &g_scheduler->fibers[fid];
    f->id = fid;
    f->state = FIBER_READY;
    f->stack = (char *)malloc(FIBER_STACK_SIZE);
    f->task = task;
    f->closure_ptr = closure_ptr;
    f->blocked_on = NULL;
    f->blocked_value = 0;
    f->saved_error = NULL;
    f->saved_current_task = task;  // fiber starts with its own task as current
    getcontext(&f->context);
    f->context.uc_stack.ss_sp = f->stack;
    f->context.uc_stack.ss_size = FIBER_STACK_SIZE;
    f->context.uc_link = &g_scheduler->scheduler_ctx;
    makecontext(&f->context, (void(*)(void))fiber_entry_fn, 1, fid);
    g_scheduler->fiber_count++;

    // Register with GC fiber stack scanner
    gc_fiber_stacks.stacks[fid].base = f->stack;
    gc_fiber_stacks.stacks[fid].size = FIBER_STACK_SIZE;
    gc_fiber_stacks.stacks[fid].active = 1;
    gc_fiber_stacks.count = g_scheduler->fiber_count;

    // Store fiber_id in task[4] for cross-referencing
    task[4] = (long)fid;

    return (long)task;
}

long __pluto_task_spawn(long closure_ptr) {
    if (!g_scheduler || g_scheduler->strategy == STRATEGY_SEQUENTIAL) {
        return task_spawn_sequential(closure_ptr);
    }
    return task_spawn_fiber(closure_ptr);
}

long __pluto_task_get(long task_ptr) {
    long *task = (long *)task_ptr;

    if (task[6] && !task[1] && !task[2]) {
        task_raise_cancelled();
        return 0;
    }

    if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
        // Fiber mode: if task not done, block and yield
        while (!task[3]) {
            Fiber *cur = &g_scheduler->fibers[g_scheduler->current_fiber];
            cur->state = FIBER_BLOCKED_TASK;
            cur->blocked_on = (void *)task;
            fiber_yield_to_scheduler();
            // Resumed — task should be done now (or we got woken spuriously)
        }
    }
    // Task is done (either was already done, or we waited)
    if (task[2]) {
        __pluto_current_error = (void *)task[2];
        return 0;
    }
    return task[1];
}

void __pluto_task_detach(long task_ptr) {
    long *task = (long *)task_ptr;
    task[5] = 1;
    if (task[3] && task[2]) {
        long *err_obj = (long *)task[2];
        char *msg_ptr = (char *)err_obj[0];
        if (msg_ptr) {
            long len = *(long *)msg_ptr;
            char *data = msg_ptr + 8;
            fprintf(stderr, "pluto: error in detached task: %.*s\n", (int)len, data);
        }
    }
}

void __pluto_task_cancel(long task_ptr) {
    long *task = (long *)task_ptr;
    task[6] = 1;
}

#else

// ── Production mode: pthread-based concurrency ──

typedef struct {
    pthread_mutex_t mutex;
    pthread_cond_t cond;
} TaskSync;

static void *__pluto_spawn_trampoline(void *arg) {
    long *task = (long *)arg;
    long closure_ptr = task[0];
    __pluto_current_error = NULL;  // clean TLS for new thread
    __pluto_current_task = task;   // set TLS for cancellation checks

    // Register this thread's stack with GC for root scanning
    int my_stack_slot = -1;
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
        pthread_mutex_lock(&gc_mutex);
        if (gc_thread_stack_count < GC_MAX_THREAD_STACKS) {
            my_stack_slot = gc_thread_stack_count++;
            gc_thread_stacks[my_stack_slot].thread = self;
            gc_thread_stacks[my_stack_slot].stack_lo = stack_lo;
            gc_thread_stacks[my_stack_slot].stack_hi = stack_hi;
            gc_thread_stacks[my_stack_slot].active = 1;
        }
        pthread_mutex_unlock(&gc_mutex);
    }

    long fn_ptr = *(long *)closure_ptr;
    long result = ((long(*)(long))fn_ptr)(closure_ptr);

    TaskSync *sync = (TaskSync *)task[4];
    pthread_mutex_lock(&sync->mutex);
    if (__pluto_current_error) {
        task[2] = (long)__pluto_current_error;
        __pluto_current_error = NULL;
    } else {
        task[1] = result;
    }
    task[3] = 1;  // done
    // If detached and errored, print to stderr
    if (task[5] && task[2]) {
        // Extract error message: error object has message field at slot 0
        long *err_obj = (long *)task[2];
        char *msg_ptr = (char *)err_obj[0];
        if (msg_ptr) {
            long len = *(long *)msg_ptr;
            char *data = msg_ptr + 8;
            fprintf(stderr, "pluto: error in detached task: %.*s\n", (int)len, data);
        }
    }
    pthread_cond_signal(&sync->cond);
    pthread_mutex_unlock(&sync->mutex);

    // Deregister thread stack from GC
    if (my_stack_slot >= 0) {
        pthread_mutex_lock(&gc_mutex);
        gc_thread_stacks[my_stack_slot].active = 0;
        pthread_mutex_unlock(&gc_mutex);
    }

    __pluto_current_task = NULL;
    atomic_fetch_sub(&__pluto_active_tasks, 1);
    return NULL;
}

long __pluto_task_spawn(long closure_ptr) {
    long *task = (long *)gc_alloc(56, GC_TAG_TASK, 3);
    task[0] = closure_ptr;
    task[1] = 0;  task[2] = 0;  task[3] = 0;
    task[5] = 0;  task[6] = 0;  // detached, cancelled

    TaskSync *sync = (TaskSync *)calloc(1, sizeof(TaskSync));
    pthread_mutex_init(&sync->mutex, NULL);
    pthread_cond_init(&sync->cond, NULL);
    task[4] = (long)sync;

    atomic_fetch_add(&__pluto_active_tasks, 1);

    pthread_t tid;
    pthread_attr_t attr;
    pthread_attr_init(&attr);
    pthread_attr_setdetachstate(&attr, PTHREAD_CREATE_DETACHED);
    int ret = pthread_create(&tid, &attr, __pluto_spawn_trampoline, task);
    pthread_attr_destroy(&attr);
    if (ret != 0) {
        fprintf(stderr, "pluto: failed to create thread: %d\n", ret);
        exit(1);
    }
    return (long)task;
}

long __pluto_task_get(long task_ptr) {
    long *task = (long *)task_ptr;
    TaskSync *sync = (TaskSync *)task[4];

    pthread_mutex_lock(&sync->mutex);
    while (!task[3]) {
        pthread_cond_wait(&sync->cond, &sync->mutex);
    }
    pthread_mutex_unlock(&sync->mutex);

    // If cancelled and no result, raise TaskCancelled
    if (task[6] && !task[1] && !task[2]) {
        task_raise_cancelled();
        return 0;
    }

    if (task[2]) {
        __pluto_current_error = (void *)task[2];
        return 0;
    }
    return task[1];
}

void __pluto_task_detach(long task_ptr) {
    long *task = (long *)task_ptr;
    TaskSync *sync = (TaskSync *)task[4];

    pthread_mutex_lock(&sync->mutex);
    task[5] = 1;  // mark as detached
    // If already done + errored, print to stderr now
    if (task[3] && task[2]) {
        long *err_obj = (long *)task[2];
        char *msg_ptr = (char *)err_obj[0];
        if (msg_ptr) {
            long len = *(long *)msg_ptr;
            char *data = msg_ptr + 8;
            fprintf(stderr, "pluto: error in detached task: %.*s\n", (int)len, data);
        }
    }
    pthread_mutex_unlock(&sync->mutex);
}

void __pluto_task_cancel(long task_ptr) {
    long *task = (long *)task_ptr;
    task[6] = 1;  // set cancelled flag
    // Wake the task thread if it's blocked on its own sync (for .get() waiters)
    TaskSync *sync = (TaskSync *)task[4];
    pthread_mutex_lock(&sync->mutex);
    pthread_cond_broadcast(&sync->cond);
    pthread_mutex_unlock(&sync->mutex);
}

#endif

// ── Deep Copy (for spawn isolation) ──────────────────────────────────────────

// Visited table for cycle detection during deep copy
typedef struct {
    void **originals;
    void **copies;
    size_t count;
    size_t cap;
} DeepCopyVisited;

static void dc_visited_init(DeepCopyVisited *v) {
    v->count = 0;
    v->cap = 16;
    v->originals = (void **)malloc(v->cap * sizeof(void *));
    v->copies    = (void **)malloc(v->cap * sizeof(void *));
}

static void dc_visited_free(DeepCopyVisited *v) {
    free(v->originals);
    free(v->copies);
}

static void *dc_visited_lookup(DeepCopyVisited *v, void *original) {
    for (size_t i = 0; i < v->count; i++) {
        if (v->originals[i] == original) return v->copies[i];
    }
    return NULL;
}

static void dc_visited_insert(DeepCopyVisited *v, void *original, void *copy) {
    if (v->count >= v->cap) {
        v->cap *= 2;
        v->originals = (void **)realloc(v->originals, v->cap * sizeof(void *));
        v->copies    = (void **)realloc(v->copies,    v->cap * sizeof(void *));
    }
    v->originals[v->count] = original;
    v->copies[v->count]    = copy;
    v->count++;
}

// Check if a value is a pointer to the start of a GC object's user data.
// Linear scan of gc_head — acceptable because spawn is not a hot path.
static GCHeader *dc_find_gc_object(void *candidate) {
    GCHeader *h = gc_head;
    while (h) {
        void *user = (char *)h + sizeof(GCHeader);
        if (user == candidate) return h;
        h = h->next;
    }
    return NULL;
}

static long dc_deep_copy_impl(long ptr, DeepCopyVisited *visited);

// Recursively deep-copy a slot value if it's a GC pointer
static long dc_copy_slot(long slot_val, DeepCopyVisited *visited) {
    if (slot_val == 0) return 0;
    GCHeader *h = dc_find_gc_object((void *)slot_val);
    if (!h) return slot_val;  // Not a GC pointer — primitive value
    return dc_deep_copy_impl(slot_val, visited);
}

static long dc_deep_copy_impl(long ptr, DeepCopyVisited *visited) {
    if (ptr == 0) return 0;

    void *orig = (void *)ptr;
    GCHeader *h = dc_find_gc_object(orig);
    if (!h) return ptr;  // Not a GC object — return as-is

    // Check visited (cycle detection)
    void *existing = dc_visited_lookup(visited, orig);
    if (existing) return (long)existing;

    switch (h->type_tag) {
    case GC_TAG_STRING:
        // Strings are immutable — no copy needed
        return ptr;

    case GC_TAG_TASK:
    case GC_TAG_CHANNEL:
        // Tasks and channels are shared by reference
        return ptr;

    case GC_TAG_OBJECT: {
        // Classes, enums, closures, errors
        // Layout: field_count * 8 bytes of slots
        uint16_t fc = h->field_count;
        void *copy = gc_alloc(h->size, GC_TAG_OBJECT, fc);
        dc_visited_insert(visited, orig, copy);
        memcpy(copy, orig, h->size);
        // Recursively deep-copy slots that are GC pointers
        long *src_slots = (long *)orig;
        long *dst_slots = (long *)copy;
        for (uint16_t i = 0; i < fc; i++) {
            dst_slots[i] = dc_copy_slot(src_slots[i], visited);
        }
        return (long)copy;
    }

    case GC_TAG_ARRAY: {
        // Handle: [len][cap][data_ptr]
        long *src = (long *)orig;
        long len = src[0];
        long cap = src[1];
        long *src_data = (long *)src[2];

        long *copy = (long *)gc_alloc(24, GC_TAG_ARRAY, 3);
        dc_visited_insert(visited, orig, copy);
        copy[0] = len;
        copy[1] = cap;
        // Allocate new data buffer (raw malloc, like __pluto_array_new)
        long *new_data = (long *)calloc((size_t)cap, sizeof(long));
        copy[2] = (long)new_data;
        // Deep-copy each element
        for (long i = 0; i < len; i++) {
            new_data[i] = dc_copy_slot(src_data[i], visited);
        }
        return (long)copy;
    }

    case GC_TAG_BYTES: {
        // Handle: [len][cap][data_ptr]
        long *src = (long *)orig;
        long len = src[0];
        long cap = src[1];
        unsigned char *src_data = (unsigned char *)src[2];

        long *copy = (long *)gc_alloc(24, GC_TAG_BYTES, 3);
        dc_visited_insert(visited, orig, copy);
        copy[0] = len;
        copy[1] = cap;
        unsigned char *new_data = (unsigned char *)calloc((size_t)cap, 1);
        memcpy(new_data, src_data, (size_t)len);
        copy[2] = (long)new_data;
        return (long)copy;
    }

    case GC_TAG_TRAIT: {
        // Handle: [data_ptr][vtable_ptr]
        long *src = (long *)orig;
        long *copy = (long *)gc_alloc(16, GC_TAG_TRAIT, 2);
        dc_visited_insert(visited, orig, copy);
        copy[0] = dc_copy_slot(src[0], visited);  // deep-copy underlying data
        copy[1] = src[1];  // vtable pointer stays the same
        return (long)copy;
    }

    case GC_TAG_MAP: {
        // Handle: [count][cap][keys_ptr][vals_ptr][meta_ptr]
        long *src = (long *)orig;
        long count = src[0];
        long cap = src[1];
        long *src_keys = (long *)src[2];
        long *src_vals = (long *)src[3];
        unsigned char *src_meta = (unsigned char *)src[4];

        long *copy = (long *)gc_alloc(40, GC_TAG_MAP, 5);
        dc_visited_insert(visited, orig, copy);
        copy[0] = count;
        copy[1] = cap;

        long *new_keys = (long *)calloc((size_t)cap, sizeof(long));
        long *new_vals = (long *)calloc((size_t)cap, sizeof(long));
        unsigned char *new_meta = (unsigned char *)calloc((size_t)cap, 1);
        memcpy(new_meta, src_meta, (size_t)cap);
        copy[2] = (long)new_keys;
        copy[3] = (long)new_vals;
        copy[4] = (long)new_meta;

        for (long i = 0; i < cap; i++) {
            if (src_meta[i] >= 0x80) {
                new_keys[i] = dc_copy_slot(src_keys[i], visited);
                new_vals[i] = dc_copy_slot(src_vals[i], visited);
            }
        }
        return (long)copy;
    }

    case GC_TAG_SET: {
        // Handle: [count][cap][keys_ptr][meta_ptr]
        long *src = (long *)orig;
        long count = src[0];
        long cap = src[1];
        long *src_keys = (long *)src[2];
        unsigned char *src_meta = (unsigned char *)src[3];

        long *copy = (long *)gc_alloc(32, GC_TAG_SET, 4);
        dc_visited_insert(visited, orig, copy);
        copy[0] = count;
        copy[1] = cap;

        long *new_keys = (long *)calloc((size_t)cap, sizeof(long));
        unsigned char *new_meta = (unsigned char *)calloc((size_t)cap, 1);
        memcpy(new_meta, src_meta, (size_t)cap);
        copy[2] = (long)new_keys;
        copy[3] = (long)new_meta;

        for (long i = 0; i < cap; i++) {
            if (src_meta[i] >= 0x80) {
                new_keys[i] = dc_copy_slot(src_keys[i], visited);
            }
        }
        return (long)copy;
    }

    default:
        // Unknown tag — return as-is
        return ptr;
    }
}

long __pluto_deep_copy(long ptr) {
    DeepCopyVisited visited;
    dc_visited_init(&visited);
    long result = dc_deep_copy_impl(ptr, &visited);
    dc_visited_free(&visited);
    return result;
}

// ── Channels ────────────────────────────────────────────────────────────────

// Channel handle layout (56 bytes, 7 slots):
//   [0] sync_ptr   (raw malloc'd ChannelSync)
//   [1] buf_ptr    (raw malloc'd circular buffer of i64)
//   [2] capacity   (int, always >= 1)
//   [3] count      (int, items in buffer)
//   [4] head       (int, read position)
//   [5] tail       (int, write position)
//   [6] closed     (int, 0 or 1)

static void chan_raise_error(const char *msg) {
    void *msg_str = __pluto_string_new((char *)msg, (long)strlen(msg));
    void *err_obj = __pluto_alloc(8);  // 1 field: message
    *(long *)err_obj = (long)msg_str;
    __pluto_raise_error(err_obj);
}

#ifdef PLUTO_TEST_MODE

// ── Test mode: channel operations (fiber-aware) ──

long __pluto_chan_create(long capacity) {
    long actual_cap = capacity > 0 ? capacity : 1;
    long *ch = (long *)gc_alloc(64, GC_TAG_CHANNEL, 0);
    ch[0] = 0;  // no sync needed in test mode
    long *buf = (long *)calloc((size_t)actual_cap, sizeof(long));
    ch[1] = (long)buf;
    ch[2] = actual_cap;
    ch[3] = 0;  // count
    ch[4] = 0;  // head
    ch[5] = 0;  // tail
    ch[6] = 0;  // closed
    ch[7] = 1;  // sender_count
    return (long)ch;
}

long __pluto_chan_send(long handle, long value) {
    long *ch = (long *)handle;

    if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
        // Record channel access for DPOR dependency tracking
        exhaustive_record_channel(g_scheduler->current_fiber, (void *)ch);

        // Fiber mode: yield when buffer is full
        while (1) {
            if (ch[6]) {
                chan_raise_error("channel closed");
                return 0;
            }
            if (ch[3] < ch[2]) {
                // Space available — push value
                long *buf = (long *)ch[1];
                buf[ch[5]] = value;
                ch[5] = (ch[5] + 1) % ch[2];
                ch[3]++;
                // Wake any fibers waiting to recv on this channel
                wake_fibers_blocked_on_chan(ch);
                wake_select_fibers_for_chan(ch);
                return value;
            }
            // Buffer full — yield
            Fiber *cur = &g_scheduler->fibers[g_scheduler->current_fiber];
            cur->state = FIBER_BLOCKED_CHAN_SEND;
            cur->blocked_on = (void *)ch;
            cur->blocked_value = value;
            fiber_yield_to_scheduler();
            // Resumed — retry
        }
    }

    // Sequential mode
    if (ch[6]) {
        chan_raise_error("channel closed");
        return 0;
    }
    if (ch[3] == ch[2]) {
        fprintf(stderr, "pluto: deadlock detected — channel send on full buffer in sequential test mode\n");
        exit(1);
    }
    long *buf = (long *)ch[1];
    buf[ch[5]] = value;
    ch[5] = (ch[5] + 1) % ch[2];
    ch[3]++;
    return value;
}

long __pluto_chan_recv(long handle) {
    long *ch = (long *)handle;

    if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
        // Record channel access for DPOR dependency tracking
        exhaustive_record_channel(g_scheduler->current_fiber, (void *)ch);

        // Fiber mode: yield when buffer is empty
        while (1) {
            if (ch[3] > 0) {
                // Data available — pop value
                long *buf = (long *)ch[1];
                long val = buf[ch[4]];
                ch[4] = (ch[4] + 1) % ch[2];
                ch[3]--;
                // Wake any fibers waiting to send on this channel
                wake_fibers_blocked_on_chan(ch);
                wake_select_fibers_for_chan(ch);
                return val;
            }
            if (ch[6]) {
                chan_raise_error("channel closed");
                return 0;
            }
            // Buffer empty — yield
            Fiber *cur = &g_scheduler->fibers[g_scheduler->current_fiber];
            cur->state = FIBER_BLOCKED_CHAN_RECV;
            cur->blocked_on = (void *)ch;
            fiber_yield_to_scheduler();
            // Resumed — retry
        }
    }

    // Sequential mode
    if (ch[3] == 0 && ch[6]) {
        chan_raise_error("channel closed");
        return 0;
    }
    if (ch[3] == 0) {
        fprintf(stderr, "pluto: deadlock detected — channel recv on empty buffer in sequential test mode\n");
        exit(1);
    }
    long *buf = (long *)ch[1];
    long val = buf[ch[4]];
    ch[4] = (ch[4] + 1) % ch[2];
    ch[3]--;
    return val;
}

long __pluto_chan_try_send(long handle, long value) {
    long *ch = (long *)handle;
    if (ch[6]) {
        chan_raise_error("channel closed");
        return 0;
    }
    if (ch[3] == ch[2]) {
        chan_raise_error("channel full");
        return 0;
    }
    long *buf = (long *)ch[1];
    buf[ch[5]] = value;
    ch[5] = (ch[5] + 1) % ch[2];
    ch[3]++;
    if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
        wake_fibers_blocked_on_chan(ch);
        wake_select_fibers_for_chan(ch);
    }
    return value;
}

long __pluto_chan_try_recv(long handle) {
    long *ch = (long *)handle;
    if (ch[3] == 0 && ch[6]) {
        chan_raise_error("channel closed");
        return 0;
    }
    if (ch[3] == 0) {
        chan_raise_error("channel empty");
        return 0;
    }
    long *buf = (long *)ch[1];
    long val = buf[ch[4]];
    ch[4] = (ch[4] + 1) % ch[2];
    ch[3]--;
    if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
        wake_fibers_blocked_on_chan(ch);
        wake_select_fibers_for_chan(ch);
    }
    return val;
}

void __pluto_chan_close(long handle) {
    long *ch = (long *)handle;
    ch[6] = 1;
    if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
        // Wake ALL fibers blocked on this channel (both send and recv)
        wake_fibers_blocked_on_chan(ch);
        wake_select_fibers_for_chan(ch);
    }
}

void __pluto_chan_sender_inc(long handle) {
    long *ch = (long *)handle;
    if (!ch) return;
    ch[7]++;
}

void __pluto_chan_sender_dec(long handle) {
    long *ch = (long *)handle;
    if (!ch) return;
    long old = ch[7];
    ch[7]--;
    if (old <= 0) {
        ch[7]++;
        return;
    }
    if (old == 1) {
        __pluto_chan_close(handle);
    }
}

#else

// ── Production mode: mutex-protected channel operations ──

long __pluto_chan_create(long capacity) {
    long actual_cap = capacity > 0 ? capacity : 1;
    // field_count=0: slots 0-1 are raw malloc ptrs, 2-7 are ints; GC_TAG_CHANNEL traces buffer
    long *ch = (long *)gc_alloc(64, GC_TAG_CHANNEL, 0);

    ChannelSync *sync = (ChannelSync *)calloc(1, sizeof(ChannelSync));
    pthread_mutex_init(&sync->mutex, NULL);
    pthread_cond_init(&sync->not_empty, NULL);
    pthread_cond_init(&sync->not_full, NULL);

    long *buf = (long *)calloc((size_t)actual_cap, sizeof(long));

    ch[0] = (long)sync;
    ch[1] = (long)buf;
    ch[2] = actual_cap;
    ch[3] = 0;  // count
    ch[4] = 0;  // head
    ch[5] = 0;  // tail
    ch[6] = 0;  // closed
    ch[7] = 1;  // sender_count (starts at 1 for the initial LetChan sender)
    return (long)ch;
}

long __pluto_chan_send(long handle, long value) {
    long *ch = (long *)handle;
    ChannelSync *sync = (ChannelSync *)ch[0];

    pthread_mutex_lock(&sync->mutex);
    while (ch[3] == ch[2] && !ch[6]) {
        pthread_cond_wait(&sync->not_full, &sync->mutex);
        // Check for task cancellation after waking from condvar
        if (__pluto_current_task && __pluto_current_task[6]) {
            pthread_mutex_unlock(&sync->mutex);
            task_raise_cancelled();
            return 0;
        }
    }
    if (ch[6]) {
        pthread_mutex_unlock(&sync->mutex);
        chan_raise_error("channel closed");
        return 0;
    }
    long *buf = (long *)ch[1];
    buf[ch[5]] = value;
    ch[5] = (ch[5] + 1) % ch[2];
    ch[3]++;
    pthread_cond_signal(&sync->not_empty);
    pthread_mutex_unlock(&sync->mutex);
    return value;
}

long __pluto_chan_recv(long handle) {
    long *ch = (long *)handle;
    ChannelSync *sync = (ChannelSync *)ch[0];

    pthread_mutex_lock(&sync->mutex);
    while (ch[3] == 0 && !ch[6]) {
        pthread_cond_wait(&sync->not_empty, &sync->mutex);
        // Check for task cancellation after waking from condvar
        if (__pluto_current_task && __pluto_current_task[6]) {
            pthread_mutex_unlock(&sync->mutex);
            task_raise_cancelled();
            return 0;
        }
    }
    if (ch[3] == 0 && ch[6]) {
        pthread_mutex_unlock(&sync->mutex);
        chan_raise_error("channel closed");
        return 0;
    }
    long *buf = (long *)ch[1];
    long val = buf[ch[4]];
    ch[4] = (ch[4] + 1) % ch[2];
    ch[3]--;
    pthread_cond_signal(&sync->not_full);
    pthread_mutex_unlock(&sync->mutex);
    return val;
}

long __pluto_chan_try_send(long handle, long value) {
    long *ch = (long *)handle;
    ChannelSync *sync = (ChannelSync *)ch[0];

    pthread_mutex_lock(&sync->mutex);
    if (ch[6]) {
        pthread_mutex_unlock(&sync->mutex);
        chan_raise_error("channel closed");
        return 0;
    }
    if (ch[3] == ch[2]) {
        pthread_mutex_unlock(&sync->mutex);
        chan_raise_error("channel full");
        return 0;
    }
    long *buf = (long *)ch[1];
    buf[ch[5]] = value;
    ch[5] = (ch[5] + 1) % ch[2];
    ch[3]++;
    pthread_cond_signal(&sync->not_empty);
    pthread_mutex_unlock(&sync->mutex);
    return value;
}

long __pluto_chan_try_recv(long handle) {
    long *ch = (long *)handle;
    ChannelSync *sync = (ChannelSync *)ch[0];

    pthread_mutex_lock(&sync->mutex);
    if (ch[3] == 0 && ch[6]) {
        pthread_mutex_unlock(&sync->mutex);
        chan_raise_error("channel closed");
        return 0;
    }
    if (ch[3] == 0) {
        pthread_mutex_unlock(&sync->mutex);
        chan_raise_error("channel empty");
        return 0;
    }
    long *buf = (long *)ch[1];
    long val = buf[ch[4]];
    ch[4] = (ch[4] + 1) % ch[2];
    ch[3]--;
    pthread_cond_signal(&sync->not_full);
    pthread_mutex_unlock(&sync->mutex);
    return val;
}

void __pluto_chan_close(long handle) {
    long *ch = (long *)handle;
    ChannelSync *sync = (ChannelSync *)ch[0];

    pthread_mutex_lock(&sync->mutex);
    ch[6] = 1;
    pthread_cond_broadcast(&sync->not_empty);
    pthread_cond_broadcast(&sync->not_full);
    pthread_mutex_unlock(&sync->mutex);
}

void __pluto_chan_sender_inc(long handle) {
    long *ch = (long *)handle;
    if (!ch) return;  // null guard for pre-declared vars
    __atomic_fetch_add(&ch[7], 1, __ATOMIC_SEQ_CST);
}

void __pluto_chan_sender_dec(long handle) {
    long *ch = (long *)handle;
    if (!ch) return;  // null guard for pre-declared vars
    long old = __atomic_fetch_sub(&ch[7], 1, __ATOMIC_SEQ_CST);
    if (old <= 0) {
        // Underflow guard: undo dec, fail safe
        __atomic_fetch_add(&ch[7], 1, __ATOMIC_SEQ_CST);
        return;
    }
    if (old == 1) {
        __pluto_chan_close(handle);  // last sender -> auto-close
    }
}

#endif

// ── Select (channel multiplexing) ──────────────────────────

/*
 * __pluto_select(buffer, count, has_default) -> case index
 *
 * Buffer layout (3 * count i64 slots):
 *   buffer[0..count)          = channel handles
 *   buffer[count..2*count)    = ops (0 = recv, 1 = send)
 *   buffer[2*count..3*count)  = values (send values in, recv values out)
 *
 * Returns:
 *   >= 0  : index of the case that completed
 *   -1    : default case (only when has_default)
 *   -2    : all channels closed (error raised via TLS)
 */
#ifdef PLUTO_TEST_MODE

// ── Test mode: select (fiber-aware) ──

static long select_try_arms(long *handles, long *ops, long *values, int n, int *indices) {
    int all_closed = 1;
    for (int si = 0; si < n; si++) {
        int i = indices[si];
        long *ch = (long *)handles[i];
        if (ops[i] == 0) {
            /* recv */
            if (ch[3] > 0) {
                long *cbuf = (long *)ch[1];
                long val = cbuf[ch[4]];
                ch[4] = (ch[4] + 1) % ch[2];
                ch[3]--;
                values[i] = val;
                if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
                    wake_fibers_blocked_on_chan(ch);
                    wake_select_fibers_for_chan(ch);
                }
                return (long)i;
            }
            if (!ch[6]) all_closed = 0;
        } else {
            /* send */
            if (!ch[6] && ch[3] < ch[2]) {
                long *cbuf = (long *)ch[1];
                cbuf[ch[5]] = values[i];
                ch[5] = (ch[5] + 1) % ch[2];
                ch[3]++;
                if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
                    wake_fibers_blocked_on_chan(ch);
                    wake_select_fibers_for_chan(ch);
                }
                return (long)i;
            }
            if (!ch[6]) all_closed = 0;
        }
    }
    if (all_closed) return -2;
    return -3;  // no ready arm, not all closed
}

long __pluto_select(long buffer_ptr, long count, long has_default) {
    long *buf = (long *)buffer_ptr;
    long *handles = &buf[0];
    long *ops     = &buf[count];
    long *values  = &buf[2 * count];

    /* Fisher-Yates shuffle for fairness */
    int indices[64];
    int n = (int)count;
    if (n > 64) n = 64;
    for (int i = 0; i < n; i++) indices[i] = i;
    unsigned long seed = (unsigned long)buffer_ptr ^ (unsigned long)__pluto_time_ns();
    for (int i = n - 1; i > 0; i--) {
        seed = seed * 6364136223846793005ULL + 1442695040888963407ULL;
        int j = (int)((seed >> 33) % (unsigned long)(i + 1));
        int tmp = indices[i]; indices[i] = indices[j]; indices[j] = tmp;
    }

    if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
        // Record all channels in this select for DPOR dependency tracking
        for (int si = 0; si < n; si++) {
            exhaustive_record_channel(g_scheduler->current_fiber, (void *)handles[si]);
        }

        // Fiber mode: loop with yield
        while (1) {
            long result = select_try_arms(handles, ops, values, n, indices);
            if (result >= 0) return result;
            if (has_default) return -1;
            if (result == -2) {
                chan_raise_error("channel closed");
                return -2;
            }
            // Block and yield
            Fiber *cur = &g_scheduler->fibers[g_scheduler->current_fiber];
            cur->state = FIBER_BLOCKED_SELECT;
            cur->blocked_on = (void *)buf;
            fiber_yield_to_scheduler();
            // Resumed — retry all arms
        }
    }

    // Sequential mode: single pass
    long result = select_try_arms(handles, ops, values, n, indices);
    if (result >= 0) return result;
    if (has_default) return -1;
    if (result == -2) {
        chan_raise_error("channel closed");
        return -2;
    }
    fprintf(stderr, "pluto: deadlock detected — select with no ready channels in sequential test mode\n");
    exit(1);
}

#else

// ── Production mode: spin-poll select ──

long __pluto_select(long buffer_ptr, long count, long has_default) {
    long *buf = (long *)buffer_ptr;
    long *handles = &buf[0];
    long *ops     = &buf[count];
    long *values  = &buf[2 * count];

    /* Fisher-Yates shuffle for fairness */
    int indices[64]; /* max 64 arms should be plenty */
    int n = (int)count;
    if (n > 64) n = 64;
    for (int i = 0; i < n; i++) indices[i] = i;
    /* simple LCG seeded from time + address entropy */
    unsigned long seed = (unsigned long)buffer_ptr ^ (unsigned long)__pluto_time_ns();

    for (int i = n - 1; i > 0; i--) {
        seed = seed * 6364136223846793005ULL + 1442695040888963407ULL;
        int j = (int)((seed >> 33) % (unsigned long)(i + 1));
        int tmp = indices[i]; indices[i] = indices[j]; indices[j] = tmp;
    }

    /* Spin-poll loop */
    long spin_us = 100;  /* start at 100 microseconds */
    for (;;) {
        int all_closed = 1;

        for (int si = 0; si < n; si++) {
            int i = indices[si];
            long *ch = (long *)handles[i];
            ChannelSync *sync = (ChannelSync *)ch[0];

            pthread_mutex_lock(&sync->mutex);

            if (ops[i] == 0) {
                /* recv */
                if (ch[3] > 0) {
                    /* data available */
                    long *cbuf = (long *)ch[1];
                    long val = cbuf[ch[4]];
                    ch[4] = (ch[4] + 1) % ch[2];
                    ch[3]--;
                    pthread_cond_signal(&sync->not_full);
                    pthread_mutex_unlock(&sync->mutex);
                    values[i] = val;
                    return (long)i;
                }
                if (!ch[6]) {
                    all_closed = 0;
                }
            } else {
                /* send */
                if (!ch[6] && ch[3] < ch[2]) {
                    /* space available */
                    long *cbuf = (long *)ch[1];
                    cbuf[ch[5]] = values[i];
                    ch[5] = (ch[5] + 1) % ch[2];
                    ch[3]++;
                    pthread_cond_signal(&sync->not_empty);
                    pthread_mutex_unlock(&sync->mutex);
                    return (long)i;
                }
                if (!ch[6]) {
                    all_closed = 0;
                }
            }

            pthread_mutex_unlock(&sync->mutex);
        }

        if (has_default) {
            return -1;
        }

        if (all_closed) {
            /* Raise ChannelClosed error */
            chan_raise_error("channel closed");
            return -2;
        }

        /* Adaptive sleep: 100us -> 200us -> ... -> 1ms max */
        usleep((useconds_t)spin_us);
        if (spin_us < 1000) spin_us = spin_us * 2;
        if (spin_us > 1000) spin_us = 1000;
    }
}

#endif

// ── Contracts ──────────────────────────────────────────────

void __pluto_invariant_violation(long class_name, long invariant_desc) {
    // class_name and invariant_desc are Pluto strings (length-prefixed)
    long *name_ptr = (long *)class_name;
    long name_len = name_ptr[0];
    char *name_data = (char *)&name_ptr[1];

    long *desc_ptr = (long *)invariant_desc;
    long desc_len = desc_ptr[0];
    char *desc_data = (char *)&desc_ptr[1];

    fprintf(stderr, "invariant violation on %.*s: %.*s\n",
            (int)name_len, name_data, (int)desc_len, desc_data);
    exit(1);
}

void __pluto_requires_violation(long fn_name, long contract_desc) {
    long *name_ptr = (long *)fn_name;
    long name_len = name_ptr[0];
    char *name_data = (char *)&name_ptr[1];

    long *desc_ptr = (long *)contract_desc;
    long desc_len = desc_ptr[0];
    char *desc_data = (char *)&desc_ptr[1];

    fprintf(stderr, "requires violation in %.*s: %.*s\n",
            (int)name_len, name_data, (int)desc_len, desc_data);
    exit(1);
}

void __pluto_ensures_violation(long fn_name, long contract_desc) {
    long *name_ptr = (long *)fn_name;
    long name_len = name_ptr[0];
    char *name_data = (char *)&name_ptr[1];

    long *desc_ptr = (long *)contract_desc;
    long desc_len = desc_ptr[0];
    char *desc_data = (char *)&desc_ptr[1];

    fprintf(stderr, "ensures violation in %.*s: %.*s\n",
            (int)name_len, name_data, (int)desc_len, desc_data);
    exit(1);
}

// ── Rwlock synchronization ─────────────────────────────────────────────────

#ifndef PLUTO_TEST_MODE
long __pluto_rwlock_init(void) {
    pthread_rwlock_t *lock = (pthread_rwlock_t *)malloc(sizeof(pthread_rwlock_t));
    pthread_rwlock_init(lock, NULL);
    return (long)lock;
}

void __pluto_rwlock_rdlock(long lock_ptr) {
    pthread_rwlock_rdlock((pthread_rwlock_t *)lock_ptr);
}

void __pluto_rwlock_wrlock(long lock_ptr) {
    pthread_rwlock_wrlock((pthread_rwlock_t *)lock_ptr);
}

void __pluto_rwlock_unlock(long lock_ptr) {
    pthread_rwlock_unlock((pthread_rwlock_t *)lock_ptr);
}
#endif

// ── Logging ────────────────────────────────────────────────────────────────

static int __pluto_global_log_level = 1;  // Default to INFO (1)

long __pluto_log_get_level(void) {
    return __pluto_global_log_level;
}

void __pluto_log_set_level(long level) {
    __pluto_global_log_level = (int)level;
}

void __pluto_log_write(void *level_str, long timestamp, void *message) {
    const char *level = (const char *)level_str + 8;
    const char *msg = (const char *)message + 8;
    fprintf(stderr, "[%s] %ld %s\n", level, timestamp, msg);
    fflush(stderr);
}

void __pluto_log_write_structured(void *level_str, long timestamp, void *message, long fields_ptr) {
    const char *level = (const char *)level_str + 8;
    const char *msg = (const char *)message + 8;
    fprintf(stderr, "[%s] %ld %s", level, timestamp, msg);
    
    long *arr_header = (long *)fields_ptr;
    long len = arr_header[0];
    long *data = (long *)arr_header[2];
    
    for (long i = 0; i < len; i++) {
        long *field_obj = (long *)data[i];
        void *key_ptr = (void *)field_obj[1];
        void *value_ptr = (void *)field_obj[2];
        const char *key = (const char *)key_ptr + 8;
        const char *value = (const char *)value_ptr + 8;
        fprintf(stderr, " %s=%s", key, value);
    }
    fprintf(stderr, "\n");
    fflush(stderr);
}

// ── Environment Variables ──────────────────────────────────────────────────

extern char **environ;

static void *__pluto_make_string(const char *c_str) {
    if (!c_str) {
        void *header = gc_alloc(8 + 1, GC_TAG_STRING, 0);
        *(long *)header = 0;
        ((char *)header)[8] = '\0';
        return header;
    }
    long len = (long)strlen(c_str);
    void *header = gc_alloc(8 + len + 1, GC_TAG_STRING, 0);
    *(long *)header = len;
    memcpy((char *)header + 8, c_str, len);
    ((char *)header)[8 + len] = '\0';
    return header;
}

void *__pluto_env_get(void *name_ptr) {
    long *name_header = (long *)name_ptr;
    long name_len = name_header[0];
    char *name_data = (char *)&name_header[1];

    char name_buf[1024];
    if (name_len >= 1024) {
        return __pluto_make_string("");
    }
    memcpy(name_buf, name_data, name_len);
    name_buf[name_len] = '\0';

    const char *val = getenv(name_buf);
    return __pluto_make_string(val);
}

void *__pluto_env_get_or(void *name_ptr, void *default_ptr) {
    long *name_header = (long *)name_ptr;
    long name_len = name_header[0];
    char *name_data = (char *)&name_header[1];

    char name_buf[1024];
    if (name_len >= 1024) {
        return default_ptr;
    }
    memcpy(name_buf, name_data, name_len);
    name_buf[name_len] = '\0';

    const char *val = getenv(name_buf);
    if (!val) {
        return default_ptr;
    }
    return __pluto_make_string(val);
}

void __pluto_env_set(void *name_ptr, void *value_ptr) {
    long *name_header = (long *)name_ptr;
    long name_len = name_header[0];
    char *name_data = (char *)&name_header[1];

    long *val_header = (long *)value_ptr;
    long val_len = val_header[0];
    char *val_data = (char *)&val_header[1];

    char name_buf[1024];
    char val_buf[4096];

    if (name_len >= 1024 || val_len >= 4096) {
        return;
    }

    memcpy(name_buf, name_data, name_len);
    name_buf[name_len] = '\0';
    memcpy(val_buf, val_data, val_len);
    val_buf[val_len] = '\0';

    setenv(name_buf, val_buf, 1);
}

long __pluto_env_exists(void *name_ptr) {
    long *name_header = (long *)name_ptr;
    long name_len = name_header[0];
    char *name_data = (char *)&name_header[1];

    char name_buf[1024];
    if (name_len >= 1024) {
        return 0;
    }
    memcpy(name_buf, name_data, name_len);
    name_buf[name_len] = '\0';

    return getenv(name_buf) != NULL ? 1 : 0;
}

void *__pluto_env_list_names() {
    // Count environment variables
    int count = 0;
    for (int i = 0; environ[i] != NULL; i++) {
        count++;
    }

    // Create array of strings
    void *arr = __pluto_array_new(count);

    for (int i = 0; i < count; i++) {
        char *env_str = environ[i];
        // Find the '=' separator
        char *eq = strchr(env_str, '=');
        if (!eq) {
            __pluto_array_push(arr, (long)__pluto_make_string(""));
            continue;
        }

        // Extract variable name (everything before '=')
        int name_len = (int)(eq - env_str);
        char name_buf[1024];
        if (name_len >= 1024) {
            __pluto_array_push(arr, (long)__pluto_make_string(""));
            continue;
        }

        memcpy(name_buf, env_str, name_len);
        name_buf[name_len] = '\0';

        __pluto_array_push(arr, (long)__pluto_make_string(name_buf));
    }

    return arr;
}

long __pluto_env_clear(void *name_ptr) {
    long *name_header = (long *)name_ptr;
    long name_len = name_header[0];
    char *name_data = (char *)&name_header[1];

    char name_buf[1024];
    if (name_len >= 1024) {
        return 0;
    }
    memcpy(name_buf, name_data, name_len);
    name_buf[name_len] = '\0';

    // unsetenv returns 0 on success, -1 on error
    return unsetenv(name_buf) == 0 ? 1 : 0;
}
