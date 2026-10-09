//──────────────────────────────────────────────────────────────────────────────
// Pluto Runtime: Garbage Collector
//
// Memory management and stop-the-world garbage collection.
//
// Design:
// - Conservative mark-and-sweep collector
// - Size-class block heap with a page map: O(1) pointer -> object lookup
// - Stop-the-world via safepoint polling (production mode)
// - Single-threaded sequential collection (test mode)
// - Supports concurrent task execution with thread stack scanning
//──────────────────────────────────────────────────────────────────────────────

#include "builtins.h"
#if defined(GC_TLH) && !defined(GC_TLAB)
#define GC_TLAB 1
#endif
#include <sys/mman.h>

// ── GC Infrastructure ─────────────────────────────────────────────────────────

// Array data buffer interval
typedef struct { void *start; void *end; void *array_handle; } GCDataInterval;

// Global GC state
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
// PLUTO_GC_VERIFY=1 cross-checks every pointer lookup that does not land on an
// object's start against a brute-force scan of the whole heap and aborts on
// any disagreement. Debug aid for collector work: it runs only on that rare
// path, so it is cheap enough to leave on for whole test suites.
static int gc_verify_enabled = -1;
#ifdef PLUTO_TEST_MODE
static int gc_collecting = 0;
#else
static atomic_int gc_collecting = 0;
#endif

// Per-collection working state lives in a collector context, reached
// through a thread-local pointer that defaults to the single global
// context. Backends that let several threads collect at once (private-heap
// collections) give each collector its own context; for everything else
// this is exactly the old set of globals. All buffers are raw malloc,
// grow-only, never GC-tracked.
//   worklist      mark stack
//   di            data-buffer interval table: the separately malloc'd
//                 backing stores of array / bytes / map / set handles,
//                 rebuilt each collection from blocks holding container
//                 handles; lets a pointer into a backing store keep its
//                 owning handle alive
//   data_min/max  coarse bounds over those buffers (one-compare reject)
//   sort_tmp      radix-sort scratch for the interval table
typedef struct GCMarkCtx {
    void **worklist;
    size_t worklist_count, worklist_cap;
    GCDataInterval *di;
    size_t di_count, di_cap;
    void *data_min, *data_max;
    GCDataInterval *sort_tmp;
    size_t sort_tmp_cap;
} GCMarkCtx;
static GCMarkCtx gc_global_ctx;
static __thread GCMarkCtx *gc_ctx = &gc_global_ctx;

#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
// Shared-heap growth since the last global collection (promotions, shared
// allocations, retired thread heaps), under gc_mutex. Global collections
// are triggered by this, not by total allocation: private garbage is the
// local collections' job.
static size_t gc_shared_growth = 0;
#define GC_TLH_SHARED_FLOOR ((size_t)4 << 20)
static size_t gc_shared_threshold = GC_TLH_SHARED_FLOOR;
#define GC_TLH_LOCAL_FLOOR  ((size_t)1 << 20)
#endif

#define gc_worklist             (gc_ctx->worklist)
#define gc_worklist_count       (gc_ctx->worklist_count)
#define gc_worklist_cap         (gc_ctx->worklist_cap)
#define gc_data_intervals       (gc_ctx->di)
#define gc_data_interval_count  (gc_ctx->di_count)
#define gc_data_interval_cap    (gc_ctx->di_cap)
#define gc_data_min             (gc_ctx->data_min)
#define gc_data_max             (gc_ctx->data_max)
#define gc_sort_tmp             (gc_ctx->sort_tmp)
#define gc_sort_tmp_cap         (gc_ctx->sort_tmp_cap)

// Which write barrier is live (runtime/builtins.h). tlh promotes on every
// store into a shared object; incr turns its deletion barrier on only for
// the duration of a marking cycle.
#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
int __pluto_gc_barrier_mode = 1;
#else
int __pluto_gc_barrier_mode = 0;
#endif

#ifdef GC_INCREMENTAL
// Snapshot-at-the-beginning log: references overwritten in or removed from
// heap objects while a cycle is marking. Each thread appends to its own
// buffer without locking; the collector drains every buffer at each step,
// when all threads are stopped.
typedef struct { long *v; size_t n, cap; } GCSatbBuf;
static __thread GCSatbBuf gc_satb_local;
static GCSatbBuf gc_satb_global;   // buffers of exited threads, under gc_mutex
static int gc_incr_marking = 0;
#define GC_INCR_STEP_BYTES ((size_t)256 << 10)   // allocation between steps
#define GC_INCR_WORK_RATIO 4                     // bytes traced per byte allocated
#define GC_INCR_SWEEP_BATCH 64                   // leftover blocks swept per allocation
static size_t gc_incr_since_step = 0;

static void gc_oom(const char *what);
static inline void gc_satb_push(GCSatbBuf *b, long v) {
    if (b->n == b->cap) {
        size_t cap = b->cap ? b->cap * 2 : 1024;
        long *grown = (long *)realloc(b->v, cap * sizeof(long));
        if (!grown) gc_oom("GC deletion log");
        b->v = grown;
        b->cap = cap;
    }
    b->v[b->n++] = v;
}

static void gc_satb_move(GCSatbBuf *dst, GCSatbBuf *src) {
    for (size_t i = 0; i < src->n; i++) gc_satb_push(dst, src->v[i]);
    src->n = 0;
}
#endif

// ── Heap: size-class blocks + page map ────────────────────────────────────────
//
// Small objects (header + user data <= GC_SMALL_MAX) live in 16 KiB blocks,
// each dedicated to one size class and carved from 4 MiB mmap'd chunks.
// Larger objects get their own page-aligned allocation rounded up to whole
// pages. Every object keeps its GCHeader immediately before its user data,
// so runtime code that reads (ptr - sizeof(GCHeader)) is unaffected.
//
// A two-level page map (4 KiB pages, 48-bit addresses) maps any address to
// the block descriptor covering it, so resolving a candidate pointer —
// start or interior — is a couple of loads and a division, with no
// per-collection index to build.
//
// Within a small block, slots [0, bump) have been handed out at least once;
// a slot that is free again carries GC_TAG_FREE in its header and is linked
// through GCHeader.next on the block's free list. Lookups reject slots past
// bump and FREE slots, so a stale or coincidental pointer into free memory
// never resolves to an object. A block with no live objects after a sweep
// returns to a shared pool and can be reused by any size class.

#define GC_PAGE_SHIFT  12
#define GC_PAGE_SIZE   ((size_t)1 << GC_PAGE_SHIFT)
#define GC_BLOCK_PAGES 4
#define GC_BLOCK_SIZE  (GC_PAGE_SIZE * GC_BLOCK_PAGES)
#define GC_SMALL_MAX   4096
#define GC_CHUNK_SIZE  ((size_t)4 << 20)
#define GC_TAG_FREE    0xFF

enum { GC_BLOCK_POOL = 0, GC_BLOCK_SMALL = 1, GC_BLOCK_LARGE = 2 };

typedef struct GCBlock {
    char *base;
    uint32_t obj_size;        // bytes per slot (small) / allocation size (large)
    uint32_t nobjs;           // slots in the block (small) / 1 (large)
    uint32_t bump;            // slots [0, bump) have been handed out
    uint8_t kind;             // GC_BLOCK_*
    uint8_t cls;              // size class (small)
    uint8_t has_containers;   // holds (or held) array/bytes/map/set handles
    uint8_t prot;             // gen: mprotect'ed read-only (holds old objects)
    uint8_t dirty;            // gen: written or allocated into since last GC
    struct GCThreadHeap *owner; // tlab/tlh: owning thread heap (NULL: shared heap)
    GCHeader *free_list;      // free slots below bump, via GCHeader.next
    struct GCBlock *next;     // class available-list / pool / descriptor freelist
} GCBlock;

static const uint16_t gc_class_sizes[] = {
    32, 48, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 448,
    512, 640, 768, 896, 1024, 1280, 1536, 1792, 2048, 2560, 3072, 3584, 4096,
};
#define GC_NUM_CLASSES (sizeof(gc_class_sizes) / sizeof(gc_class_sizes[0]))
static uint8_t gc_size_class[GC_SMALL_MAX / 16 + 1];  // [(total + 15) / 16] -> class
static int gc_classes_ready = 0;

static GCBlock *gc_class_avail[GC_NUM_CLASSES];  // blocks that may have room
static GCBlock *gc_block_pool = NULL;            // empty blocks, any class
static GCBlock **gc_small_blocks = NULL;         // every small/pool block (sweep)
static size_t gc_small_block_count = 0, gc_small_block_cap = 0;
static GCBlock **gc_large_blocks = NULL;         // live large objects (sweep)
static size_t gc_large_count = 0, gc_large_cap = 0;
static GCBlock *gc_desc_free = NULL;             // recycled large descriptors
static char *gc_chunk_cur = NULL, *gc_chunk_end = NULL;

// Coarse bounds over all heap memory (chunks and large allocations), only
// ever widened. A candidate outside them is rejected before the page map.
static uintptr_t gc_heap_lo = UINTPTR_MAX, gc_heap_hi = 0;

static inline void gc_heap_widen(char *base, size_t bytes) {
    if ((uintptr_t)base < gc_heap_lo) gc_heap_lo = (uintptr_t)base;
    if ((uintptr_t)base + bytes > gc_heap_hi) gc_heap_hi = (uintptr_t)base + bytes;
}

#define GC_PM_LEAF_BITS 18
#define GC_PM_ROOT_BITS 18   // 12 + 18 + 18 = 48-bit user address space
#define GC_PM_LEAF_MASK (((uintptr_t)1 << GC_PM_LEAF_BITS) - 1)
static GCBlock **gc_pagemap[(size_t)1 << GC_PM_ROOT_BITS];

static void gc_oom(const char *what) {
    fprintf(stderr, "pluto: out of memory (%s)\n", what);
    exit(1);
}

static inline int gc_is_container_tag(uint8_t tag) {
    return tag == GC_TAG_ARRAY || tag == GC_TAG_BYTES || tag == GC_TAG_MAP || tag == GC_TAG_SET;
}

// Release stores: thread-local collections (tlh) read the page map without
// gc_mutex while other threads extend it, and must see a fully initialized
// leaf / block descriptor behind any pointer they load (acquire, in
// gc_pagemap_get_acq).
static void gc_pagemap_set(char *addr, size_t npages, GCBlock *b) {
    uintptr_t pg = (uintptr_t)addr >> GC_PAGE_SHIFT;
    for (size_t k = 0; k < npages; k++, pg++) {
        GCBlock **leaf = gc_pagemap[pg >> GC_PM_LEAF_BITS];
        if (!leaf) {
            if (!b) continue;   // clearing a page that was never mapped
            leaf = (GCBlock **)calloc((size_t)1 << GC_PM_LEAF_BITS, sizeof(GCBlock *));
            if (!leaf) gc_oom("GC page map");
            __atomic_store_n(&gc_pagemap[pg >> GC_PM_LEAF_BITS], leaf, __ATOMIC_RELEASE);
        }
        __atomic_store_n(&leaf[pg & GC_PM_LEAF_MASK], b, __ATOMIC_RELEASE);
    }
}

static inline GCBlock *gc_pagemap_get_acq(uintptr_t a) {
    if (a >> 48) return NULL;
    uintptr_t pg = a >> GC_PAGE_SHIFT;
    GCBlock **leaf = __atomic_load_n(&gc_pagemap[pg >> GC_PM_LEAF_BITS], __ATOMIC_ACQUIRE);
    return leaf ? __atomic_load_n(&leaf[pg & GC_PM_LEAF_MASK], __ATOMIC_ACQUIRE) : NULL;
}

static inline GCBlock *gc_pagemap_get(uintptr_t a) {
    if (a >> 48) return NULL;
    uintptr_t pg = a >> GC_PAGE_SHIFT;
    GCBlock **leaf = gc_pagemap[pg >> GC_PM_LEAF_BITS];
    return leaf ? leaf[pg & GC_PM_LEAF_MASK] : NULL;
}

static void gc_init_classes(void) {
    size_t c = 0;
    for (size_t i = 0; i <= GC_SMALL_MAX / 16; i++) {
        while (gc_class_sizes[c] < i * 16) c++;
        gc_size_class[i] = (uint8_t)c;
    }
    gc_classes_ready = 1;
}

static GCBlock *gc_new_small_block(size_t cls) {
    GCBlock *b = gc_block_pool;
    if (b) {
        gc_block_pool = b->next;
    } else {
        if (gc_chunk_cur == gc_chunk_end) {
            void *m = mmap(NULL, GC_CHUNK_SIZE, PROT_READ | PROT_WRITE,
                           MAP_PRIVATE | MAP_ANON, -1, 0);
            if (m == MAP_FAILED) gc_oom("GC heap");
            gc_chunk_cur = (char *)m;
            gc_chunk_end = (char *)m + GC_CHUNK_SIZE;
            gc_heap_widen(gc_chunk_cur, GC_CHUNK_SIZE);
        }
        b = (GCBlock *)calloc(1, sizeof(GCBlock));
        if (!b) gc_oom("GC block descriptor");
        b->base = gc_chunk_cur;
        gc_chunk_cur += GC_BLOCK_SIZE;
        if (gc_small_block_count == gc_small_block_cap) {
            size_t cap = gc_small_block_cap ? gc_small_block_cap * 2 : 256;
            GCBlock **grown = (GCBlock **)realloc(gc_small_blocks, cap * sizeof(GCBlock *));
            if (!grown) gc_oom("GC block table");
            gc_small_blocks = grown;
            gc_small_block_cap = cap;
        }
        gc_small_blocks[gc_small_block_count++] = b;
        gc_pagemap_set(b->base, GC_BLOCK_PAGES, b);
    }
    b->kind = GC_BLOCK_SMALL;
    b->cls = (uint8_t)cls;
    b->obj_size = gc_class_sizes[cls];
    b->nobjs = (uint32_t)(GC_BLOCK_SIZE / b->obj_size);
    b->bump = 0;
    b->free_list = NULL;
    b->has_containers = 0;
    b->prot = 0;
    b->dirty = 0;
    b->owner = NULL;
    b->next = NULL;
    return b;
}

#ifdef GC_GENERATIONAL
// Generational, non-moving (--gc gen). Ages use sticky mark bits: after a
// collection every survivor keeps mark = 1 and is "old"; new objects start
// at 0. A minor collection marks only young objects, from the roots plus a
// remembered set; tracing stops at old objects (already marked). The write
// barrier is the MMU: after each collection every block holding old
// objects is mprotect'ed read-only, and the first store into one faults; the
// handler unprotects the block and marks it dirty. A minor collection then
// re-traces every old object in a dirty block, plus every old container and
// channel (their backing stores are malloc'd, outside the protected heap, so
// stores into them are invisible to the barrier). Allocation unprotects a
// block before writing into it. A major collection (when the old generation
// has doubled since the last one) unprotects everything, clears all marks
// and collects the whole heap.
#define GC_GEN_NURSERY        ((size_t)8 << 20)
#define GC_GEN_MIN_OLD_LIMIT  ((size_t)16 << 20)
static size_t gc_gen_old_limit = GC_GEN_MIN_OLD_LIMIT;
static int gc_gen_major = 0;            // current collection is major
// mprotect works on whole VM pages: 16 KiB on Apple Silicon, 4 KiB on most
// x86-64. Large objects are aligned and padded to it so each owns its pages.
// If the VM page is larger than a block, blocks can't be protected one by
// one: the barrier is disabled and every collection is a full one.
static size_t gc_gen_vm_page = 4096;
static int gc_gen_barrier_off = 0;
static long gc_gen_minor_count = 0, gc_gen_major_count = 0;
static struct sigaction gc_gen_prev_segv, gc_gen_prev_bus;

static inline size_t gc_block_span(GCBlock *b) {
    return b->kind == GC_BLOCK_LARGE ? b->obj_size : GC_BLOCK_SIZE;
}

static void gc_gen_set_prot(char *base, size_t len, int readonly) {
    if (mprotect(base, len, readonly ? PROT_READ : (PROT_READ | PROT_WRITE)) != 0) {
        fprintf(stderr, "pluto: gc: mprotect(%p, %zu, %s) failed: %s\n", (void *)base, len, readonly ? "RO" : "RW", strerror(errno));
        abort();
    }
}

static inline void gc_gen_unprotect(GCBlock *b) {
    if (b->prot) {
        gc_gen_set_prot(b->base, gc_block_span(b), 0);
        b->prot = 0;
    }
    b->dirty = 1;
}

static void gc_gen_fault(int sig, siginfo_t *info, void *ctx) {
    GCBlock *b = gc_pagemap_get((uintptr_t)info->si_addr);
    if (b && (b->kind == GC_BLOCK_SMALL || b->kind == GC_BLOCK_LARGE)) {
        // Heap blocks are only ever read-only because of the barrier, so any
        // fault inside one is a barrier fault. prot may already be clear:
        // another thread faulted on the same block first and unprotected it
        // while this fault was in flight. Then there is nothing to do but
        // retry the store (which now succeeds).
        if (b->prot) {
            mprotect(b->base, gc_block_span(b), PROT_READ | PROT_WRITE);
            b->prot = 0;
        }
        b->dirty = 1;
        return;   // retry the store
    }
    // Not a barrier fault: hand it to whatever was installed before us.
    struct sigaction *prev = sig == SIGBUS ? &gc_gen_prev_bus : &gc_gen_prev_segv;
    if (prev->sa_flags & SA_SIGINFO) {
        if (prev->sa_sigaction) { prev->sa_sigaction(sig, info, ctx); return; }
    } else if (prev->sa_handler != SIG_DFL && prev->sa_handler != SIG_IGN && prev->sa_handler) {
        prev->sa_handler(sig);
        return;
    }
    signal(sig, SIG_DFL);   // re-executing the access now takes the default action
}

static void gc_gen_install_barrier(void) {
    long pg = sysconf(_SC_PAGESIZE);
    if (pg > 0) gc_gen_vm_page = (size_t)pg;
    if (gc_gen_vm_page > GC_BLOCK_SIZE) {
        gc_gen_barrier_off = 1;
        return;
    }
    struct sigaction sa;
    memset(&sa, 0, sizeof sa);
    sa.sa_sigaction = gc_gen_fault;
    sa.sa_flags = SA_SIGINFO | SA_RESTART;
    sigemptyset(&sa.sa_mask);
    sigaction(SIGSEGV, &sa, &gc_gen_prev_segv);
    sigaction(SIGBUS, &sa, &gc_gen_prev_bus);
}
#endif

#ifdef GC_LAZY_SWEEP
static GCBlock *gc_class_unswept[GC_NUM_CLASSES];
static size_t gc_marked_bytes;
static size_t gc_sweep_block(GCBlock *b);
static size_t gc_slot_bytes(GCHeader *h);
#endif

static GCHeader *gc_small_alloc(size_t cls, GCBlock **out_block) {
    GCBlock *b = gc_class_avail[cls];
    while (b && !b->free_list && b->bump == b->nobjs) {
        b = gc_class_avail[cls] = b->next;   // full: drop from the list
    }
#ifdef GC_LAZY_SWEEP
    while (!b && gc_class_unswept[cls]) {
        GCBlock *u = gc_class_unswept[cls];
        gc_class_unswept[cls] = u->next;
        gc_sweep_block(u);           // -> pool, or onto gc_class_avail[cls]
        b = gc_class_avail[cls];
    }
#endif
    if (!b) {
        b = gc_new_small_block(cls);
        gc_class_avail[cls] = b;
    }
#ifdef GC_GENERATIONAL
    if (b->prot || !b->dirty) gc_gen_unprotect(b);
#endif
    GCHeader *h;
    if (b->free_list) {
        h = b->free_list;
        b->free_list = h->next;
    } else {
        h = (GCHeader *)(b->base + (size_t)b->bump * b->obj_size);
        b->bump++;
    }
    *out_block = b;
    return h;
}

static GCHeader *gc_large_alloc(size_t total, size_t *out_bytes, GCBlock **out_block) {
    size_t align = GC_PAGE_SIZE;
#ifdef GC_GENERATIONAL
    if (gc_gen_vm_page > align) align = gc_gen_vm_page;
#endif
    size_t bytes = (total + align - 1) & ~(align - 1);
    void *m = NULL;
    if (posix_memalign(&m, align, bytes) != 0) gc_oom("GC heap");
    GCBlock *b = gc_desc_free;
    if (b) {
        gc_desc_free = b->next;
    } else {
        b = (GCBlock *)malloc(sizeof(GCBlock));
        if (!b) gc_oom("GC block descriptor");
    }
    memset(b, 0, sizeof(*b));
    b->base = (char *)m;
    b->obj_size = (uint32_t)bytes;
    b->nobjs = 1;
    b->bump = 1;
    b->kind = GC_BLOCK_LARGE;
    if (gc_large_count == gc_large_cap) {
        size_t cap = gc_large_cap ? gc_large_cap * 2 : 64;
        GCBlock **grown = (GCBlock **)realloc(gc_large_blocks, cap * sizeof(GCBlock *));
        if (!grown) gc_oom("GC block table");
        gc_large_blocks = grown;
        gc_large_cap = cap;
    }
    gc_large_blocks[gc_large_count++] = b;
    gc_pagemap_set(b->base, bytes >> GC_PAGE_SHIFT, b);
    gc_heap_widen(b->base, bytes);
    *out_bytes = bytes;
    *out_block = b;
    return (GCHeader *)m;
}

// Allocate and zero a heap object; returns its user pointer. Callers hold
// gc_mutex in production mode.
static void *gc_obj_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count) {
    if (!gc_classes_ready) gc_init_classes();
    size_t total = sizeof(GCHeader) + user_size;
    GCBlock *b;
    GCHeader *h;
    size_t slot_bytes;
    if (total <= GC_SMALL_MAX) {
        size_t cls = gc_size_class[(total + 15) >> 4];
        h = gc_small_alloc(cls, &b);
        slot_bytes = gc_class_sizes[cls];
    } else {
        h = gc_large_alloc(total, &slot_bytes, &b);
    }
    memset(h, 0, total);
    h->size = (uint32_t)user_size;
    h->type_tag = type_tag;
    h->field_count = field_count;
#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
    h->next = GC_SHARED_TAG;   // the shared heap's objects are shared
    gc_shared_growth += slot_bytes;
#endif
    if (gc_is_container_tag(type_tag)) b->has_containers = 1;
    gc_bytes_allocated += slot_bytes;
#ifdef GC_INCREMENTAL
    if (gc_incr_marking) {   // allocated black: survives the cycle in progress
        h->mark = 1;
        gc_marked_bytes += slot_bytes;
    }
#endif
    return (char *)h + sizeof(GCHeader);
}

#if defined(GC_TLAB) && !defined(PLUTO_TEST_MODE)
// ── Thread heaps (--gc tlab and up) ──────────────────────────────────────────
//
// Every registered thread owns a heap: per-size-class available lists over
// blocks it owns, plus an allocation counter. The fast path pops a slot from
// the thread's own lists without any lock; the thread reports its allocation
// to the global counter only every GC_TLAB_BUDGET bytes, so gc_mutex is taken
// once per budget instead of once per object. New blocks and large objects
// take the slow path under gc_mutex. A global collection (all threads
// stopped) folds every heap's unreported bytes into the global count and
// sweeps each block back onto its owner's lists. When a thread exits, its
// blocks pass to the shared heap. Threads that never registered (and test
// mode) allocate from the shared heap under gc_mutex as before.
#define GC_TLAB_BUDGET ((size_t)256 << 10)

typedef struct GCThreadHeap {
    GCBlock *avail[GC_NUM_CLASSES];
    GCBlock **blocks;          // every block this heap owns (small and large)
    size_t nblocks, cap;
    size_t unreported;         // slot bytes allocated since the last report
#if defined(GC_TLH)
    GCMarkCtx ctx;             // this thread's promotion / local-collection context
    size_t promoted_bytes;     // promoted to shared since the last report
    size_t local_alloc;        // private bytes allocated since the last local collection
    size_t local_threshold;    // local collection when local_alloc reaches this
    size_t local_freed;        // reclaimed by local collections, not yet reported
    size_t private_live;       // private bytes live after the last collection
    GCBlock *empty;            // blocks emptied by local collections (kind POOL,
                               // still owned here) until handed back under gc_mutex
    GCBlock *dead_large;       // large objects freed by local collections, unmapped
                               // under gc_mutex
    long torture_count;
    long local_count;
#endif
} GCThreadHeap;

static GCThreadHeap **gc_tlh_heaps = NULL;   // registry, under gc_mutex
static size_t gc_tlh_heap_count = 0, gc_tlh_heap_cap = 0;
static __thread GCThreadHeap *gc_my_heap = NULL;

static void gc_tlh_own(GCThreadHeap *H, GCBlock *b) {   // under gc_mutex or STW
    b->owner = H;
    if (H->nblocks == H->cap) {
        size_t cap = H->cap ? H->cap * 2 : 64;
        GCBlock **grown = (GCBlock **)realloc(H->blocks, cap * sizeof(GCBlock *));
        if (!grown) gc_oom("GC thread heap");
        H->blocks = grown;
        H->cap = cap;
    }
    H->blocks[H->nblocks++] = b;
}

// Lock-free private allocation; NULL when the size class has no room (or
// the object is large), meaning the caller must take the slow path.
static inline void *gc_tlh_alloc_fast(GCThreadHeap *H, size_t user_size, uint8_t type_tag,
                                      uint16_t field_count) {
    size_t total = sizeof(GCHeader) + user_size;
    if (total > GC_SMALL_MAX) return NULL;
    if (!gc_classes_ready) return NULL;
    size_t cls = gc_size_class[(total + 15) >> 4];
    GCBlock *b = H->avail[cls];
    while (b && !b->free_list && b->bump == b->nobjs) b = H->avail[cls] = b->next;
    if (!b) return NULL;
    GCHeader *h;
    if (b->free_list) {
        h = b->free_list;
        b->free_list = h->next;
    } else {
        h = (GCHeader *)(b->base + (size_t)b->bump * b->obj_size);
        b->bump++;
    }
    memset(h, 0, total);
    h->size = (uint32_t)user_size;
    h->type_tag = type_tag;
    h->field_count = field_count;
    if (gc_is_container_tag(type_tag)) b->has_containers = 1;
    H->unreported += gc_class_sizes[cls];
#if defined(GC_TLH)
    H->local_alloc += gc_class_sizes[cls];
#endif
    return (char *)h + sizeof(GCHeader);
}

// Slow path, under gc_mutex: a fresh block for H's size class, or a large
// object owned by H.
static void *gc_tlh_alloc_slow(GCThreadHeap *H, size_t user_size, uint8_t type_tag,
                               uint16_t field_count) {
    if (!gc_classes_ready) gc_init_classes();
    size_t total = sizeof(GCHeader) + user_size;
    if (total <= GC_SMALL_MAX) {
        void *u = gc_tlh_alloc_fast(H, user_size, type_tag, field_count);  // a sweep may have refilled
        if (u) return u;
        size_t cls = gc_size_class[(total + 15) >> 4];
#if defined(GC_TLH)
        // Prefer a block this heap emptied itself (no pool traffic).
        GCBlock *b = H->empty;
        if (b) {
            H->empty = b->next;
            b->kind = GC_BLOCK_SMALL;
            b->cls = (uint8_t)cls;
            b->obj_size = gc_class_sizes[cls];
            b->nobjs = (uint32_t)(GC_BLOCK_SIZE / b->obj_size);
            b->bump = 0;
            b->free_list = NULL;
            b->has_containers = 0;
            b->next = NULL;
        } else {
            b = gc_new_small_block(cls);
        }
#else
        GCBlock *b = gc_new_small_block(cls);
#endif
        gc_tlh_own(H, b);
        b->next = H->avail[cls];
        H->avail[cls] = b;
        return gc_tlh_alloc_fast(H, user_size, type_tag, field_count);
    }
    size_t bytes;
    GCBlock *b;
    GCHeader *h = gc_large_alloc(total, &bytes, &b);
    gc_tlh_own(H, b);
    memset(h, 0, total);
    h->size = (uint32_t)user_size;
    h->type_tag = type_tag;
    h->field_count = field_count;
    if (gc_is_container_tag(type_tag)) b->has_containers = 1;
    gc_bytes_allocated += bytes;
#if defined(GC_TLH)
    H->local_alloc += bytes;
#endif
    return (char *)h + sizeof(GCHeader);
}

#if defined(GC_TLH)
// Under gc_mutex (or stop-the-world): fold what H did without the lock into
// the global state. Allocation and local reclamation go into the global
// byte count, promotions into shared growth; blocks the local collector
// emptied return to the pool and large objects it freed are unmapped.
static void gc_tlh_report(GCThreadHeap *H) {
    gc_bytes_allocated += H->unreported;
    H->unreported = 0;
    size_t f = H->local_freed < gc_bytes_allocated ? H->local_freed : gc_bytes_allocated;
    gc_bytes_allocated -= f;
    H->local_freed = 0;
    gc_shared_growth += H->promoted_bytes;
    H->promoted_bytes = 0;
    while (H->empty) {
        GCBlock *b = H->empty;
        H->empty = b->next;
        __atomic_store_n(&b->owner, NULL, __ATOMIC_RELAXED);
        b->next = gc_block_pool;
        gc_block_pool = b;
    }
    if (H->dead_large) {
        // Dead large objects carry GC_TAG_FREE; drop them from the table.
        for (size_t k = 0; k < gc_large_count;) {
            GCBlock *b = gc_large_blocks[k];
            if (((GCHeader *)b->base)->type_tag == GC_TAG_FREE) {
                gc_large_blocks[k] = gc_large_blocks[--gc_large_count];
            } else {
                k++;
            }
        }
        while (H->dead_large) {
            GCBlock *b = H->dead_large;
            H->dead_large = b->next;
            gc_pagemap_set(b->base, b->obj_size >> GC_PAGE_SHIFT, NULL);
            free(b->base);
            b->next = gc_desc_free;
            gc_desc_free = b;
        }
    }
}
#endif
#endif

// Resolve p to the object whose user data starts at p, or (when interior is
// set) also any object whose user data contains p. NULL for anything else:
// unmapped addresses, pool blocks, free or never-used slots, headers,
// padding past an object's size.
static inline GCHeader *gc_lookup(void *p, int interior) {
    uintptr_t a = (uintptr_t)p;
    if (a < gc_heap_lo || a >= gc_heap_hi) return NULL;
    GCBlock *b = gc_pagemap_get(a);
    if (!b || b->kind == GC_BLOCK_POOL) return NULL;
    char *slot;
    if (b->kind == GC_BLOCK_SMALL) {
        size_t idx = (size_t)(a - (uintptr_t)b->base) / b->obj_size;
        if (idx >= b->bump) return NULL;
        slot = b->base + idx * b->obj_size;
    } else {
        slot = b->base;
    }
    GCHeader *h = (GCHeader *)slot;
    if (h->type_tag == GC_TAG_FREE) return NULL;
    char *user = slot + sizeof(GCHeader);
    if ((char *)p == user) return h;
    if (!interior) return NULL;
    return ((char *)p > user && (char *)p < user + h->size) ? h : NULL;
}

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
#ifdef GC_INCREMENTAL
    GCSatbBuf *satb;   // the thread's deletion log, drained at each step
#endif
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
#ifdef GC_INCREMENTAL
    slot->satb = &gc_satb_local;
#endif
    gc_active_thread_count++;
    gc_my_slot = slot;
    // Flag and slot flip together under gc_mutex: the collector (which also
    // holds gc_mutex to count) can never see one without the other
    gc_thread_registered = 1;
#if defined(GC_TLAB)
    {
        GCThreadHeap *H = (GCThreadHeap *)calloc(1, sizeof(GCThreadHeap));
        if (!H) gc_oom("GC thread heap");
        if (gc_tlh_heap_count == gc_tlh_heap_cap) {
            size_t cap = gc_tlh_heap_cap ? gc_tlh_heap_cap * 2 : 16;
            GCThreadHeap **grown =
                (GCThreadHeap **)realloc(gc_tlh_heaps, cap * sizeof(GCThreadHeap *));
            if (!grown) gc_oom("GC thread heap");
            gc_tlh_heaps = grown;
            gc_tlh_heap_cap = cap;
        }
        gc_tlh_heaps[gc_tlh_heap_count++] = H;
#if defined(GC_TLH)
        H->local_threshold = GC_TLH_LOCAL_FLOOR;
#endif
        gc_my_heap = H;
    }
#endif
    pthread_mutex_unlock(&gc_mutex);
}

#if defined(GC_TLAB)
// Under gc_mutex: hand every block of H to the shared heap and drop H.
static void gc_tlh_retire(GCThreadHeap *H) {
#if defined(GC_TLH)
    gc_tlh_report(H);
#else
    gc_bytes_allocated += H->unreported;
#endif
    for (size_t k = 0; k < H->nblocks; k++) {
        GCBlock *b = H->blocks[k];
        __atomic_store_n(&b->owner, NULL, __ATOMIC_RELAXED);
#if defined(GC_TLH)
        // The shared heap holds only shared objects: what is still private
        // becomes shared, and counts as shared growth. After an exit reclaim
        // nothing private is left; this matters for heaps retired without
        // one (fork ghosts).
        uint32_t n = b->kind == GC_BLOCK_LARGE ? 1 : b->bump;
        size_t osz = b->kind == GC_BLOCK_LARGE ? 0 : b->obj_size;
        for (uint32_t i = 0; i < n; i++) {
            GCHeader *h = (GCHeader *)(b->base + (size_t)i * osz);
            if (h->type_tag != GC_TAG_FREE && h->next != GC_SHARED_TAG) {
                h->next = GC_SHARED_TAG;
                gc_shared_growth += b->kind == GC_BLOCK_LARGE ? b->obj_size : osz;
            }
        }
#endif
        if (b->kind == GC_BLOCK_SMALL && (b->free_list || b->bump < b->nobjs)) {
            b->next = gc_class_avail[b->cls];
            gc_class_avail[b->cls] = b;
        }
    }
    for (size_t i = 0; i < gc_tlh_heap_count; i++) {
        if (gc_tlh_heaps[i] == H) {
            gc_tlh_heaps[i] = gc_tlh_heaps[--gc_tlh_heap_count];
            break;
        }
    }
    free(H->blocks);
#if defined(GC_TLH)
    free(H->ctx.worklist);
    free(H->ctx.di);
    free(H->ctx.sort_tmp);
#endif
    free(H);
}
#endif

#if defined(GC_TLH)
static void gc_tlh_exit_reclaim(GCThreadHeap *H);
#endif

void __pluto_gc_deregister_thread_stack(void) {
#if defined(GC_TLH)
    // Still registered, outside gc_mutex: free everything private in bulk.
    if (gc_my_heap) gc_tlh_exit_reclaim(gc_my_heap);
#endif
    gc_heap_lock();
#if defined(GC_TLAB)
    if (gc_my_heap) {
        gc_tlh_retire(gc_my_heap);
        gc_my_heap = NULL;
    }
#endif
    if (gc_my_slot) {
#ifdef GC_INCREMENTAL
        gc_satb_move(&gc_satb_global, &gc_satb_local);   // nothing logged is lost
        gc_my_slot->satb = NULL;
#endif
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
#ifdef GC_INCREMENTAL
                if (gc_thread_stacks[i]->satb) gc_satb_move(&gc_satb_global, gc_thread_stacks[i]->satb);
                gc_thread_stacks[i]->satb = NULL;
#endif
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
#if defined(GC_TLAB)
        // Heaps of threads that did not survive the fork pass to the shared heap.
        for (size_t i = gc_tlh_heap_count; i-- > 0;) {
            if (gc_tlh_heaps[i] != gc_my_heap) gc_tlh_retire(gc_tlh_heaps[i]);
        }
#endif
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
    // A global is reachable from every thread: share what it points to.
    __pluto_gc_promote_store((long)*(void **)slot);
#ifndef PLUTO_TEST_MODE
    pthread_mutex_unlock(&gc_mutex);
#endif
}

// Get GC header from user pointer
static inline GCHeader *gc_get_header(void *user_ptr) {
    return (GCHeader *)((char *)user_ptr - sizeof(GCHeader));
}

// ── Allocation ────────────────────────────────────────────────────────────────

// PLUTO_GC_TORTURE=N forces a collection every N allocations. Missing roots
// and missing barriers are timing-dependent bugs a normal threshold rarely
// hits; collecting constantly hits them. Verification aid (design R6) —
// run whole test suites with it, ideally together with PLUTO_GC_VERIFY=1.
static long gc_torture_every = -1;
static long gc_torture_count = 0;

static inline int gc_torture_due(void) {
    if (gc_torture_every < 0) {
        const char *e = getenv("PLUTO_GC_TORTURE");
        gc_torture_every = e ? atol(e) : 0;
        if (gc_torture_every < 0) gc_torture_every = 0;
    }
    return gc_torture_every > 0 && ++gc_torture_count % gc_torture_every == 0;
}

static void gc_init_env(void) {
    if (gc_log_enabled >= 0) return;
    const char *v = getenv("PLUTO_GC_VERIFY");
    gc_verify_enabled = (v && v[0] == '1') ? 1 : 0;
    const char *e = getenv("PLUTO_GC_LOG");
    gc_log_enabled = (e && e[0] == '1') ? 1 : 0;
}

// PLUTO_GC_VERIFY: overwrite a dead object's data so a use after free reads
// garbage (and usually crashes) instead of plausible stale values.
static inline void gc_poison(GCHeader *h) {
    if (gc_verify_enabled > 0) memset((char *)h + sizeof(GCHeader), 0xA5, h->size);
}

#ifdef GC_INCREMENTAL
static void gc_incr_start(void);
static void gc_incr_step(void);
static int gc_sweep_some(size_t max_blocks);
#endif

#ifdef PLUTO_TEST_MODE
void *gc_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count) {
#ifdef GC_INCREMENTAL
    if (gc_stack_bottom && !gc_collecting) {
        int torture = gc_torture_due();
        if (gc_incr_marking) {
            gc_incr_since_step += user_size + sizeof(GCHeader);
            if (torture || gc_incr_since_step >= GC_INCR_STEP_BYTES) gc_incr_step();
        } else if ((torture || gc_bytes_allocated + user_size + sizeof(GCHeader) > gc_threshold)
                   && gc_sweep_some(GC_INCR_SWEEP_BATCH)) {
            gc_incr_start();
        }
    }
    return gc_obj_alloc(user_size, type_tag, field_count);
#endif
    // Test mode: single-threaded, no mutex needed
    if (gc_stack_bottom && !gc_collecting
        && (gc_bytes_allocated + user_size + sizeof(GCHeader) > gc_threshold
            || gc_torture_due())) {
        __pluto_gc_collect();
    }
    return gc_obj_alloc(user_size, type_tag, field_count);
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

#if defined(GC_TLH)
static void gc_tlh_local_collect(GCThreadHeap *H);

// Under gc_mutex. A global collection is due when the shared heap has grown
// past its threshold — or, as a backstop against accounting drift, when the
// whole heap has grown to four times the last global collection's target.
static int gc_tlh_global_due(void) {
    return gc_shared_growth > gc_shared_threshold || gc_bytes_allocated > 4 * gc_threshold;
}

// Allocation for a thread with a heap. The fast path is lock-free; every
// GC_TLAB_BUDGET bytes (or when a size class runs dry) the thread reaches a
// checkpoint: it runs a local collection if its private allocation is due
// (still lock-free), then takes gc_mutex to report, collects globally if
// the shared heap is due, and refills. Torture mode runs a local collection
// every N allocations and a global one every 8N.
static void *gc_tlh_alloc(GCThreadHeap *H, size_t user_size, uint8_t type_tag,
                          uint16_t field_count) {
    int local_due = 0, global_due = 0;
    if (gc_torture_every < 0) (void)gc_torture_due();   // read PLUTO_GC_TORTURE once
    if (gc_torture_every > 0) {
        long n = ++H->torture_count;
        local_due = n % gc_torture_every == 0;
        global_due = n % (8 * gc_torture_every) == 0;
    }
    void *u = gc_tlh_alloc_fast(H, user_size, type_tag, field_count);
    if (u && !local_due && !global_due && H->unreported < GC_TLAB_BUDGET) return u;
    if (gc_stack_bottom && (local_due || H->local_alloc >= H->local_threshold)) {
        gc_tlh_local_collect(H);   // u (if any) stays rooted from this frame
        if (!u) u = gc_tlh_alloc_fast(H, user_size, type_tag, field_count);
    }
    gc_heap_lock();
    gc_tlh_report(H);
    if (gc_stack_bottom && (global_due || gc_tlh_global_due())) {
        int stopped = gc_stw_stop_threads();
        __pluto_gc_collect();
        gc_stw_resume_threads(stopped);
    }
    if (!u) u = gc_tlh_alloc_slow(H, user_size, type_tag, field_count);
    pthread_mutex_unlock(&gc_mutex);
    return u;
}
#endif

void *gc_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count) {
#if defined(GC_TLH)
    GCThreadHeap *H = gc_my_heap;
    if (H) return gc_tlh_alloc(H, user_size, type_tag, field_count);
#elif defined(GC_TLAB)
    // Thread-local fast path (no lock). Torture mode routes every allocation
    // through the slow path so it can count them.
    GCThreadHeap *H = gc_my_heap;
    if (H && gc_torture_every <= 0) {
        void *u = gc_tlh_alloc_fast(H, user_size, type_tag, field_count);
        if (u && H->unreported < GC_TLAB_BUDGET) return u;
        if (u) {
            // Budget spent: report it, and collect if the heap is due. u
            // stays rooted through the collection from this frame.
            gc_heap_lock();
            gc_bytes_allocated += H->unreported;
            H->unreported = 0;
            if (gc_stack_bottom && gc_bytes_allocated > gc_threshold) {
                int stopped = gc_stw_stop_threads();
                __pluto_gc_collect();
                gc_stw_resume_threads(stopped);
            }
            pthread_mutex_unlock(&gc_mutex);
            return u;
        }
    }
#endif
    // The wait for gc_mutex counts as a safe region: the collector holds it
    // for the whole collection, and a thread parked here must count as
    // stopped or stop-the-world would deadlock.
    gc_heap_lock();
#if defined(GC_TLAB)
    if (H) {
        gc_bytes_allocated += H->unreported;
        H->unreported = 0;
    }
#endif
#ifdef GC_INCREMENTAL
    if (gc_stack_bottom) {
        int torture = gc_torture_due();
        if (gc_incr_marking) {
            gc_incr_since_step += user_size + sizeof(GCHeader);
            if (torture || gc_incr_since_step >= GC_INCR_STEP_BYTES) {
                int stopped = gc_stw_stop_threads();
                gc_incr_step();
                gc_stw_resume_threads(stopped);
            }
        } else if ((torture || gc_bytes_allocated + user_size + sizeof(GCHeader) > gc_threshold)
                   && gc_sweep_some(GC_INCR_SWEEP_BATCH)) {
            // The previous cycle's leftover sweep runs first, in batches
            // and without stopping anyone; the cycle starts once it is done.
            int stopped = gc_stw_stop_threads();
            gc_incr_start();
            gc_stw_resume_threads(stopped);
        }
    }
#else
    if (gc_stack_bottom
        && (gc_bytes_allocated + user_size + sizeof(GCHeader) > gc_threshold
            || gc_torture_due())) {
        // Initiation is serialized by gc_mutex: whoever holds it and sees the
        // threshold exceeded collects. A thread that was parked waiting on
        // gc_mutex during a collection re-checks the (now raised) threshold
        // and usually just allocates.
        int stopped = gc_stw_stop_threads();
        __pluto_gc_collect();
        gc_stw_resume_threads(stopped);
    }
#endif
#if defined(GC_TLAB)
    void *user = H ? gc_tlh_alloc_slow(H, user_size, type_tag, field_count)
                   : gc_obj_alloc(user_size, type_tag, field_count);
#else
    void *user = gc_obj_alloc(user_size, type_tag, field_count);
#endif
    pthread_mutex_unlock(&gc_mutex);
    return user;
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

// ── Data-buffer interval table ────────────────────────────────────────────────

static int gc_data_interval_cmp(const void *a, const void *b) {
    const GCDataInterval *ia = (const GCDataInterval *)a;
    const GCDataInterval *ib = (const GCDataInterval *)b;
    if (ia->start < ib->start) return -1;
    if (ia->start > ib->start) return 1;
    return 0;
}

// Sorted by start for the owner binary search. qsort through a comparator
// function pointer is slow on big tables, so sort with an LSD radix sort on
// the address offset instead: key = (start - min) >> 4 (malloc results are
// 16-byte aligned), 11 bits per pass. Correctness never depends on it: the
// result is verified with a linear sortedness check and re-sorted with
// qsort if that fails or the scratch buffer can't be allocated.
#define GC_RADIX_BITS 11
#define GC_RADIX_BUCKETS (1u << GC_RADIX_BITS)
#define GC_RADIX_MIN_N 256   // below this, qsort is already cheap


static void gc_sort_data_intervals(GCDataInterval *a, size_t n) {
    if (n < 2) return;
    if (n >= GC_RADIX_MIN_N) {
        if (n > gc_sort_tmp_cap) {
            GCDataInterval *grown =
                (GCDataInterval *)realloc(gc_sort_tmp, n * sizeof(GCDataInterval));
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

            GCDataInterval *src = a, *dst = gc_sort_tmp;
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
                GCDataInterval *t = src;
                src = dst;
                dst = t;
            }
            if (src != a) memcpy(a, src, n * sizeof(GCDataInterval));

            int sorted = 1;
            for (size_t i = 1; i < n; i++) {
                if (a[i - 1].start > a[i].start) { sorted = 0; break; }
            }
            if (sorted) return;
        }
    }
    qsort(a, n, sizeof(GCDataInterval), gc_data_interval_cmp);
}

static inline void gc_data_interval_add(void *start, void *end, void *owner) {
    if (gc_data_interval_count == gc_data_interval_cap) {
        size_t cap = gc_data_interval_cap ? gc_data_interval_cap * 2 : 1024;
        GCDataInterval *grown =
            (GCDataInterval *)realloc(gc_data_intervals, cap * sizeof(GCDataInterval));
        if (!grown) gc_oom("GC lookup table");
        gc_data_intervals = grown;
        gc_data_interval_cap = cap;
    }
    GCDataInterval *d = &gc_data_intervals[gc_data_interval_count++];
    d->start = start;
    d->end = end;
    d->array_handle = owner;
}

static void gc_add_container_intervals(GCHeader *h) {
    void *user = (char *)h + sizeof(GCHeader);
    switch (h->type_tag) {
    case GC_TAG_ARRAY:   // handle [len][cap][data_ptr], 8 bytes per element
    case GC_TAG_BYTES: { // handle [len][cap][data_ptr], 1 byte per element
        if (h->size < 24) break;
        long *handle = (long *)user;
        long cap = handle[1];
        char *data = (char *)handle[2];
        if (data && cap > 0) {
            long elem = h->type_tag == GC_TAG_ARRAY ? 8 : 1;
            gc_data_interval_add(data, data + cap * elem, user);
        }
        break;
    }
    case GC_TAG_MAP: {   // [count][cap][keys_ptr][vals_ptr][meta_ptr]
        if (h->size < 40) break;
        long *mh = (long *)user;
        long cap = mh[1];
        if (cap <= 0) break;
        char *keys = (char *)mh[2], *vals = (char *)mh[3], *meta = (char *)mh[4];
        if (keys) gc_data_interval_add(keys, keys + cap * 8, user);
        if (vals) gc_data_interval_add(vals, vals + cap * 8, user);
        if (meta) gc_data_interval_add(meta, meta + cap, user);
        break;
    }
    case GC_TAG_SET: {   // [count][cap][keys_ptr][meta_ptr]
        if (h->size < 32) break;
        long *sh = (long *)user;
        long cap = sh[1];
        if (cap <= 0) break;
        char *keys = (char *)sh[2], *meta = (char *)sh[3];
        if (keys) gc_data_interval_add(keys, keys + cap * 8, user);
        if (meta) gc_data_interval_add(meta, meta + cap, user);
        break;
    }
    default:
        break;
    }
}

static void gc_finish_data_intervals(void);

// Rebuild the data-buffer table. Only blocks flagged as holding container
// handles are scanned (the flag is refreshed by every sweep), plus any large
// container objects.
static void gc_build_data_intervals(void) {
    gc_data_interval_count = 0;
    for (size_t k = 0; k < gc_small_block_count; k++) {
        GCBlock *b = gc_small_blocks[k];
        if (b->kind != GC_BLOCK_SMALL || !b->has_containers) continue;
        for (uint32_t i = 0; i < b->bump; i++) {
            GCHeader *h = (GCHeader *)(b->base + (size_t)i * b->obj_size);
            if (gc_is_container_tag(h->type_tag)) gc_add_container_intervals(h);
        }
    }
    for (size_t k = 0; k < gc_large_count; k++) {
        GCHeader *h = (GCHeader *)gc_large_blocks[k]->base;
        if (gc_is_container_tag(h->type_tag)) gc_add_container_intervals(h);
    }
    gc_finish_data_intervals();
}

// Sort the collected intervals and compute their coarse bounds.
static void gc_finish_data_intervals(void) {
    gc_sort_data_intervals(gc_data_intervals, gc_data_interval_count);
    if (gc_data_interval_count == 0) {
        gc_data_min = NULL;
        gc_data_max = NULL;
    } else {
        void *lo = gc_data_intervals[0].start;
        void *hi = NULL;
        for (size_t k = 0; k < gc_data_interval_count; k++) {
            if (gc_data_intervals[k].end > hi) hi = gc_data_intervals[k].end;
        }
        gc_data_min = lo;
        gc_data_max = hi;
    }
}

#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
// Set while this thread traces its own heap without stopping anyone: a local
// collection, or a promotion. Every lookup is then confined to H's private
// objects (gc_tlh_find_own), which is what makes both safe to run while
// other threads mutate the heap metadata of their own blocks.
static __thread GCThreadHeap *gc_local_heap = NULL;
static __thread int gc_promoting = 0;

// candidate -> H's private object containing it (start or interior), else
// NULL. Lock-free: the page map and the owner field are read atomically; a
// block whose owner is H is only ever changed by this thread or by a
// stop-the-world collection (which cannot be running while this thread is),
// so its fields are stable once ownership is confirmed. The address range
// is re-checked against the descriptor because a large-object descriptor
// can be recycled for a different allocation between the two loads.
static GCHeader *gc_tlh_find_own(void *candidate, GCThreadHeap *H) {
    uintptr_t a = (uintptr_t)candidate;
    GCBlock *b = gc_pagemap_get_acq(a);
    if (!b || __atomic_load_n(&b->owner, __ATOMIC_RELAXED) != H) return NULL;
    char *slot;
    if (b->kind == GC_BLOCK_SMALL) {
        if (a < (uintptr_t)b->base || a >= (uintptr_t)b->base + GC_BLOCK_SIZE) return NULL;
        size_t idx = (size_t)(a - (uintptr_t)b->base) / b->obj_size;
        if (idx >= b->bump) return NULL;
        slot = b->base + idx * b->obj_size;
    } else if (b->kind == GC_BLOCK_LARGE) {
        if (a < (uintptr_t)b->base || a >= (uintptr_t)b->base + b->obj_size) return NULL;
        slot = b->base;
    } else {
        return NULL;
    }
    GCHeader *h = (GCHeader *)slot;
    if (h->type_tag == GC_TAG_FREE || h->next == GC_SHARED_TAG) return NULL;
    char *user = slot + sizeof(GCHeader);
    if ((char *)candidate == user) return h;
    return ((char *)candidate > user && (char *)candidate < user + h->size) ? h : NULL;
}

// PLUTO_GC_VERIFY for local lookups: brute force over H's own blocks.
static void gc_tlh_verify_own(void *candidate, GCThreadHeap *H, GCHeader *found) {
    GCHeader *want = NULL;
    for (size_t k = 0; k < H->nblocks && !want; k++) {
        GCBlock *b = H->blocks[k];
        uint32_t n = b->kind == GC_BLOCK_LARGE ? 1 : (b->kind == GC_BLOCK_SMALL ? b->bump : 0);
        size_t osz = b->kind == GC_BLOCK_LARGE ? 0 : b->obj_size;
        for (uint32_t i = 0; i < n; i++) {
            GCHeader *h = (GCHeader *)(b->base + (size_t)i * osz);
            if (h->type_tag == GC_TAG_FREE || h->next == GC_SHARED_TAG) continue;
            char *u = (char *)h + sizeof(GCHeader);
            if ((char *)candidate == u
                || ((char *)candidate > u && (char *)candidate < u + h->size)) {
                want = h;
                break;
            }
        }
    }
    if (want != found) {
        fprintf(stderr, "pluto: PLUTO_GC_VERIFY: local lookup of %p returned %p, heap "
                "scan found %p\n", candidate, (void *)found, (void *)want);
        abort();
    }
}
#endif

// Find the GC object containing candidate (start or interior pointer).
static GCHeader *gc_find_object(void *candidate) {
#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
    if (gc_local_heap) {
        GCHeader *h = gc_tlh_find_own(candidate, gc_local_heap);
        // Cross-check only candidates on H's pages that did not land on an
        // object start; everything else cannot be H's and is rejected above.
        if (gc_verify_enabled > 0 && !gc_promoting
            && h != (GCHeader *)((char *)candidate - sizeof(GCHeader))) {
            GCBlock *vb = gc_pagemap_get_acq((uintptr_t)candidate);
            if (vb && vb->owner == gc_local_heap)
                gc_tlh_verify_own(candidate, gc_local_heap, h);
        }
        return h;
    }
#endif
    GCHeader *found = gc_lookup(candidate, 1);
    // Verify only candidates inside the heap that did not land on an object's
    // start (the common hit), so the brute force stays rare.
    // Only during collections: at mutator time (promotion tracing) other
    // threads run, and a brute-force scan would race with them.
    if (gc_verify_enabled > 0 && gc_collecting
        && (uintptr_t)candidate >= gc_heap_lo && (uintptr_t)candidate < gc_heap_hi
        && found != (GCHeader *)((char *)candidate - sizeof(GCHeader))) {
        // Brute force over every allocated object.
        GCHeader *want = NULL;
        for (size_t k = 0; k < gc_small_block_count && !want; k++) {
            GCBlock *b = gc_small_blocks[k];
            if (b->kind != GC_BLOCK_SMALL) continue;
            for (uint32_t i = 0; i < b->bump; i++) {
                GCHeader *h = (GCHeader *)(b->base + (size_t)i * b->obj_size);
                if (h->type_tag == GC_TAG_FREE) continue;
                char *u = (char *)h + sizeof(GCHeader);
                if ((char *)candidate == u
                    || ((char *)candidate > u && (char *)candidate < u + h->size)) {
                    want = h;
                    break;
                }
            }
        }
        for (size_t k = 0; k < gc_large_count && !want; k++) {
            GCHeader *h = (GCHeader *)gc_large_blocks[k]->base;
            char *u = (char *)h + sizeof(GCHeader);
            if ((char *)candidate == u
                || ((char *)candidate > u && (char *)candidate < u + h->size)) {
                want = h;
            }
        }
        if (want != found) {
            fprintf(stderr, "pluto: PLUTO_GC_VERIFY: lookup of %p returned %p, heap scan "
                    "found %p\n", candidate, (void *)found, (void *)want);
            abort();
        }
    }
    return found;
}

// Binary search: find array handle owning a data buffer containing candidate
static void *gc_find_array_owner(void *candidate) {
    if (gc_data_interval_count == 0) return NULL;
    if (candidate < gc_data_min || candidate >= gc_data_max) return NULL;
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

#if defined(GC_PARALLEL_MARK) && !defined(PLUTO_TEST_MODE)
// Parallel marking (--gc parmark). Roots are scanned serially; the resulting
// worklist moves to a shared stack and helper threads (plus the collector)
// drain it in parallel. Each worker traces from a private stack, claiming
// objects with an atomic exchange on the mark byte, and spills half its
// stack to the shared one whenever another worker is idle. Marking ends when
// every worker is idle and the shared stack is empty. Helpers are created
// lazily (cores - 1, at most GC_PM_MAX_HELPERS, or PLUTO_GC_THREADS - 1),
// park between collections, and are never registered as mutator threads.
#define GC_PM_MAX_HELPERS 7
#define GC_PM_SHARE 4         // spill when a stack is deeper than this (DFS keeps
                              // stacks near tree depth, so this must be small)...
#define GC_PM_BATCH 64        // ...and take this many from the shared stack
typedef struct { void **items; size_t count, cap; } GCMarkStack;
static __thread GCMarkStack *gc_pm_local = NULL;   // set while marking in parallel
static GCMarkStack gc_pm_stacks[GC_PM_MAX_HELPERS + 1];
static pthread_mutex_t gc_pm_mu = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t gc_pm_work_cv = PTHREAD_COND_INITIALIZER;
static pthread_cond_t gc_pm_start_cv = PTHREAD_COND_INITIALIZER;
static pthread_cond_t gc_pm_done_cv = PTHREAD_COND_INITIALIZER;
static void **gc_pm_shared = NULL;
static size_t gc_pm_shared_count = 0, gc_pm_shared_cap = 0;
static int gc_pm_nhelpers = -1;           // -1: not started
static int gc_pm_active = 0, gc_pm_idle = 0, gc_pm_done = 0, gc_pm_finished = 0;
static long gc_pm_epoch = 0;
static pid_t gc_pm_pid = 0;

static inline void gc_pm_push(GCMarkStack *st, void *p) {
    if (st->count == st->cap) {
        size_t cap = st->cap ? st->cap * 2 : 1024;
        void **grown = (void **)realloc(st->items, cap * sizeof(void *));
        if (!grown) gc_oom("GC mark stack");
        st->items = grown;
        st->cap = cap;
    }
    st->items[st->count++] = p;
}

static void gc_pm_shared_push(void *p) {   // caller holds gc_pm_mu
    if (gc_pm_shared_count == gc_pm_shared_cap) {
        size_t cap = gc_pm_shared_cap ? gc_pm_shared_cap * 2 : 1024;
        void **grown = (void **)realloc(gc_pm_shared, cap * sizeof(void *));
        if (!grown) gc_oom("GC mark stack");
        gc_pm_shared = grown;
        gc_pm_shared_cap = cap;
    }
    gc_pm_shared[gc_pm_shared_count++] = p;
}
#endif

#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
static void gc_tlh_promote_visit(GCHeader *h);
#endif

static void gc_mark_object(void *user_ptr) {
    GCHeader *h = gc_get_header(user_ptr);
#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
    if (gc_promoting) {   // tracing for promotion, not for a collection
        gc_tlh_promote_visit(h);
        return;
    }
#endif
    if (h->mark) return;
#if defined(GC_PARALLEL_MARK) && !defined(PLUTO_TEST_MODE)
    if (gc_pm_local) {
        if (__atomic_exchange_n(&h->mark, 1, __ATOMIC_RELAXED)) return;   // lost the race
        gc_pm_push(gc_pm_local, user_ptr);
        return;
    }
#endif
    h->mark = 1;
#ifdef GC_LAZY_SWEEP
    gc_marked_bytes += gc_slot_bytes(h);
#endif
    gc_worklist_push(user_ptr);
}

#ifdef GC_INCREMENTAL
// Large containers are scanned GC_INCR_CHUNK slots at a time while marking
// incrementally, so no single step pays for a whole million-element array.
// A continuation (container, next slot) waits on gc_conts; each chunk
// re-reads the container's length and buffers, so pushes, growth and
// reallocation in between are harmless. Entries that move toward lower
// slots — behind the cursor — are logged by the runtime while marking
// (remove_at, reverse, map/set deletion shifts and rehashing), so nothing
// reachable at the snapshot can slip past an in-progress scan.
#define GC_INCR_CHUNK 1024
typedef struct { void *obj; long next; } GCCont;
static GCCont *gc_conts = NULL;
static size_t gc_cont_count = 0, gc_cont_cap = 0;

static inline void gc_shade(long word) {
    GCHeader *c = gc_find_object((void *)word);
    if (c) gc_mark_object((char *)c + sizeof(GCHeader));
}

// Scan slots [from, from + GC_INCR_CHUNK) of an array, map or set; queue a
// continuation if more remain. Returns the number of slots scanned.
static long gc_scan_container_chunk(void *user_ptr, long from) {
    GCHeader *h = gc_get_header(user_ptr);
    long *s = (long *)user_ptr;
    long limit, end;
    if (h->type_tag == GC_TAG_ARRAY) {
        long *data = (long *)s[2];
        limit = s[0];
        end = limit < from + GC_INCR_CHUNK ? limit : from + GC_INCR_CHUNK;
        for (long i = from; i < end; i++) gc_shade(data[i]);
    } else {
        int is_map = h->type_tag == GC_TAG_MAP;
        long *keys = (long *)s[2];
        long *vals = is_map ? (long *)s[3] : NULL;
        unsigned char *meta = (unsigned char *)(is_map ? s[4] : s[3]);
        limit = s[1];
        end = limit < from + GC_INCR_CHUNK ? limit : from + GC_INCR_CHUNK;
        for (long i = from; i < end; i++) {
            if (meta[i] < 0x80) continue;
            gc_shade(keys[i]);
            if (vals) gc_shade(vals[i]);
        }
    }
    if (end < limit) {
        if (gc_cont_count == gc_cont_cap) {
            size_t cap = gc_cont_cap ? gc_cont_cap * 2 : 64;
            GCCont *grown = (GCCont *)realloc(gc_conts, cap * sizeof(GCCont));
            if (!grown) gc_oom("GC scan continuations");
            gc_conts = grown;
            gc_cont_cap = cap;
        }
        gc_conts[gc_cont_count].obj = user_ptr;
        gc_conts[gc_cont_count].next = end;
        gc_cont_count++;
    }
    return end > from ? end - from : 0;
}
#endif

static void gc_trace_object(void *user_ptr) {
    GCHeader *h = gc_get_header(user_ptr);
#ifdef GC_INCREMENTAL
    if (gc_incr_marking && (h->type_tag == GC_TAG_ARRAY || h->type_tag == GC_TAG_MAP
                            || h->type_tag == GC_TAG_SET)) {
        gc_scan_container_chunk(user_ptr, 0);
        return;
    }
#endif
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
            if (child) {
                if (!child->mark) gc_mark_object((char *)child + sizeof(GCHeader));
                continue;  // disjoint from data buffers (see gc_mark_candidate)
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

#if defined(GC_PARALLEL_MARK) && !defined(PLUTO_TEST_MODE)
static void gc_pm_work(GCMarkStack *st) {
    gc_pm_local = st;
    for (;;) {
        while (st->count) {
            void *o = st->items[--st->count];
            gc_trace_object(o);
            if (st->count > GC_PM_SHARE && __atomic_load_n(&gc_pm_idle, __ATOMIC_RELAXED) > 0) {
                pthread_mutex_lock(&gc_pm_mu);
                size_t give = st->count / 2;
                for (size_t i = 0; i < give; i++) gc_pm_shared_push(st->items[i]);
                memmove(st->items, st->items + give, (st->count - give) * sizeof(void *));
                st->count -= give;
                pthread_cond_broadcast(&gc_pm_work_cv);
                pthread_mutex_unlock(&gc_pm_mu);
            }
        }
        pthread_mutex_lock(&gc_pm_mu);
        for (;;) {
            if (gc_pm_shared_count > 0) {
                size_t take = gc_pm_shared_count < GC_PM_BATCH ? gc_pm_shared_count : GC_PM_BATCH;
                for (size_t i = 0; i < take; i++) gc_pm_push(st, gc_pm_shared[--gc_pm_shared_count]);
                break;
            }
            if (gc_pm_done) break;
            gc_pm_idle++;
            if (gc_pm_idle == gc_pm_active) {   // nobody holds work: marking is complete
                gc_pm_done = 1;
                pthread_cond_broadcast(&gc_pm_work_cv);
                break;
            }
            pthread_cond_wait(&gc_pm_work_cv, &gc_pm_mu);
            gc_pm_idle--;
        }
        int finished = gc_pm_done && st->count == 0;
        pthread_mutex_unlock(&gc_pm_mu);
        if (finished) break;
    }
    gc_pm_local = NULL;
}

static void *gc_pm_helper_main(void *arg) {
    int id = (int)(intptr_t)arg;
    long seen = 0;
    for (;;) {
        pthread_mutex_lock(&gc_pm_mu);
        while (gc_pm_epoch == seen) pthread_cond_wait(&gc_pm_start_cv, &gc_pm_mu);
        seen = gc_pm_epoch;
        pthread_mutex_unlock(&gc_pm_mu);
        gc_pm_work(&gc_pm_stacks[id]);
        pthread_mutex_lock(&gc_pm_mu);
        gc_pm_finished++;
        pthread_cond_signal(&gc_pm_done_cv);
        pthread_mutex_unlock(&gc_pm_mu);
    }
    return NULL;
}

static void gc_pm_start_helpers(void) {
    pid_t pid = getpid();
    if (gc_pm_nhelpers >= 0 && gc_pm_pid == pid) return;
    if (gc_pm_nhelpers >= 0) {
        // Forked child: the helpers did not survive; start over cleanly.
        pthread_mutex_init(&gc_pm_mu, NULL);
        pthread_cond_init(&gc_pm_work_cv, NULL);
        pthread_cond_init(&gc_pm_start_cv, NULL);
        pthread_cond_init(&gc_pm_done_cv, NULL);
        gc_pm_epoch = 0;
    }
    gc_pm_pid = pid;
    long n = sysconf(_SC_NPROCESSORS_ONLN) - 1;
    const char *env = getenv("PLUTO_GC_THREADS");
    if (env && atoi(env) > 0) n = atoi(env) - 1;
    if (n < 0) n = 0;
    if (n > GC_PM_MAX_HELPERS) n = GC_PM_MAX_HELPERS;
    gc_pm_nhelpers = 0;
    for (int i = 1; i <= n; i++) {
        pthread_t t;
        pthread_attr_t attr;
        pthread_attr_init(&attr);
        pthread_attr_setdetachstate(&attr, PTHREAD_CREATE_DETACHED);
        if (pthread_create(&t, &attr, gc_pm_helper_main, (void *)(intptr_t)i) == 0) gc_pm_nhelpers++;
        pthread_attr_destroy(&attr);
    }
}

// Drain the serial worklist (filled by root scanning) in parallel.
// Below this much allocated heap a mark finishes in a millisecond or two,
// and waking and synchronizing helpers costs more than it saves: mark
// serially.
#define GC_PM_MIN_HEAP ((size_t)32 << 20)

static void gc_pm_drain(void) {
    if (gc_bytes_allocated < GC_PM_MIN_HEAP) {
        while (gc_worklist_count > 0) gc_trace_object(gc_worklist[--gc_worklist_count]);
        return;
    }
    gc_pm_start_helpers();
    if (gc_pm_nhelpers == 0) {
        while (gc_worklist_count > 0) gc_trace_object(gc_worklist[--gc_worklist_count]);
        return;
    }
    pthread_mutex_lock(&gc_pm_mu);
    while (gc_worklist_count > 0) gc_pm_shared_push(gc_worklist[--gc_worklist_count]);
    gc_pm_active = gc_pm_nhelpers + 1;
    gc_pm_idle = 0;
    gc_pm_done = 0;
    gc_pm_finished = 0;
    gc_pm_epoch++;
    pthread_cond_broadcast(&gc_pm_start_cv);
    pthread_mutex_unlock(&gc_pm_mu);

    gc_pm_work(&gc_pm_stacks[0]);

    pthread_mutex_lock(&gc_pm_mu);
    while (gc_pm_finished < gc_pm_nhelpers) pthread_cond_wait(&gc_pm_done_cv, &gc_pm_mu);
    pthread_mutex_unlock(&gc_pm_mu);
}
#endif

// Heap checkers (PLUTO_GC_VERIFY): call check(parent, word, where) for every
// pointer-sized slot of h that can hold a reference, per its tag.
static void gc_check_children(GCHeader *h, void (*check)(GCHeader *, long, const char *)) {
    long *slots = (long *)(h + 1);
    switch (h->type_tag) {
    case GC_TAG_STRING:
    case GC_TAG_BYTES:
        return;
    case GC_TAG_ARRAY: {
        long len = slots[0], *data = (long *)slots[2];
        for (long i = 0; data && i < len; i++) check(h, data[i], "array element");
        return;
    }
    case GC_TAG_MAP: {
        long cap = slots[1], *keys = (long *)slots[2], *vals = (long *)slots[3];
        unsigned char *meta = (unsigned char *)slots[4];
        for (long i = 0; meta && i < cap; i++) {
            if (meta[i] < 0x80) continue;
            check(h, keys[i], "map key");
            check(h, vals[i], "map value");
        }
        return;
    }
    case GC_TAG_SET: {
        long cap = slots[1], *keys = (long *)slots[2];
        unsigned char *meta = (unsigned char *)slots[3];
        for (long i = 0; meta && i < cap; i++) {
            if (meta[i] >= 0x80) check(h, keys[i], "set element");
        }
        return;
    }
    case GC_TAG_TRAIT:
    case GC_TAG_STRING_SLICE:
        check(h, slots[0], "data pointer");
        return;
    case GC_TAG_CHANNEL: {
        long *buf = (long *)slots[1], cap = slots[2], count = slots[3], head = slots[4];
        for (long i = 0; buf && cap > 0 && i < count; i++) check(h, buf[(head + i) % cap], "channel buffer");
        return;
    }
    default:
        for (uint16_t i = 0; i < h->field_count; i++) check(h, slots[i], "field");
        return;
    }
}

// ── Promotion (--gc tlh) ─────────────────────────────────────────────────────
#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
// Invariant I: no shared object holds a pointer to a private object.
// Promotion keeps it: before a pointer is stored into a shared object (or
// handed across a thread boundary) the stored value's transitive closure is
// made shared — in place, by tagging headers; nothing moves. A thread only
// ever promotes its own private objects (it cannot reach another thread's).
// Tracing is conservative, so an integer that looks like a private pointer
// may be over-promoted: safe, it only delays reclamation.
static void gc_tlh_promote_visit(GCHeader *h) {
    GCThreadHeap *H = gc_my_heap;
    GCBlock *b = gc_pagemap_get((uintptr_t)h);
    if (!b || b->owner != H || h->next == GC_SHARED_TAG) return;
    h->next = GC_SHARED_TAG;
    H->promoted_bytes += b->kind == GC_BLOCK_LARGE ? b->obj_size : b->obj_size;
    gc_worklist_push((char *)h + sizeof(GCHeader));
}

void __pluto_gc_promote_store(long value) {
    GCThreadHeap *H = gc_my_heap;
    if (!H) return;   // unregistered threads allocate shared objects only
    GCHeader *h = gc_tlh_find_own((void *)value, H);
    if (!h) return;   // already shared, another heap's, or not an object
    GCMarkCtx *saved = gc_ctx;
    gc_ctx = &H->ctx;
    gc_promoting = 1;
    gc_local_heap = H;   // children resolve through gc_tlh_find_own too
    gc_worklist_count = 0;
    gc_tlh_promote_visit(h);
    while (gc_worklist_count > 0) gc_trace_object(gc_worklist[--gc_worklist_count]);
    gc_local_heap = NULL;
    gc_promoting = 0;
    gc_ctx = saved;
}

// PLUTO_GC_VERIFY: check invariant I at a global collection (all threads
// stopped). Every pointer slot of every shared object is resolved by exact
// start (to keep integers that fall inside an object from raising false
// alarms); landing on a private object is a missed barrier.
static void gc_tlh_check_slot(GCHeader *parent, long word, const char *where) {
    GCHeader *c = gc_lookup((void *)word, 0);
    if (!c || c->next == GC_SHARED_TAG) return;
    GCBlock *cb = gc_pagemap_get((uintptr_t)c);
    if (!cb || !cb->owner) return;
    fprintf(stderr, "pluto: PLUTO_GC_VERIFY: invariant I violated: shared object %p "
            "(tag %d, size %u) %s holds private object %p (tag %d, size %u)\n",
            (void *)(parent + 1), parent->type_tag, parent->size, where,
            (void *)(c + 1), c->type_tag, c->size);
    abort();
}

static void gc_tlh_check_object(GCHeader *h) {
    gc_check_children(h, gc_tlh_check_slot);
}

static void gc_tlh_check_invariant(void) {
    for (size_t k = 0; k < gc_small_block_count; k++) {
        GCBlock *b = gc_small_blocks[k];
        if (b->kind != GC_BLOCK_SMALL) continue;
        for (uint32_t i = 0; i < b->bump; i++) {
            GCHeader *h = (GCHeader *)(b->base + (size_t)i * b->obj_size);
            if (h->type_tag != GC_TAG_FREE && h->next == GC_SHARED_TAG) gc_tlh_check_object(h);
        }
    }
    for (size_t k = 0; k < gc_large_count; k++) {
        GCHeader *h = (GCHeader *)gc_large_blocks[k]->base;
        if (h->next == GC_SHARED_TAG) gc_tlh_check_object(h);
    }
}
#else
void __pluto_gc_promote_store(long value) { (void)value; }
#endif

static void gc_mark_candidate(void *candidate) {
    // Check if candidate points into a GC object
    GCHeader *h = gc_find_object(candidate);
    if (h) {
        if (!h->mark) gc_mark_object((char *)h + sizeof(GCHeader));
        // Object user-regions and data buffers are disjoint allocations, so a
        // candidate inside an object cannot also be inside a data buffer.
        return;
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

// ── Sweep ────────────────────────────────────────────────────────────────────

// Release the non-GC resources owned by a dead object.
static void gc_finalize(GCHeader *h) {
    long *slots = (long *)((char *)h + sizeof(GCHeader));
    switch (h->type_tag) {
    case GC_TAG_ARRAY:   // [len][cap][data_ptr]
    case GC_TAG_BYTES:
        if (h->size >= 24 && slots[2]) free((void *)slots[2]);
        break;
    case GC_TAG_MAP:     // [count][cap][keys_ptr][vals_ptr][meta_ptr]
        if (h->size >= 40) {
            if (slots[2]) free((void *)slots[2]);
            if (slots[3]) free((void *)slots[3]);
            if (slots[4]) free((void *)slots[4]);
        }
        break;
    case GC_TAG_SET:     // [count][cap][keys_ptr][meta_ptr]
        if (h->size >= 32) {
            if (slots[2]) free((void *)slots[2]);
            if (slots[3]) free((void *)slots[3]);
        }
        break;
    case GC_TAG_TASK:
        // Test mode: slots[4] holds the FIBER ID, not a TaskSync pointer —
        // freeing it would be free(small int). Nothing to release there.
#ifndef PLUTO_TEST_MODE
        if (h->size >= 56 && slots[4]) {
            void *sync = (void *)slots[4];
            pthread_mutex_destroy((pthread_mutex_t *)sync);
            pthread_cond_destroy((pthread_cond_t *)((char *)sync + sizeof(pthread_mutex_t)));
            free(sync);
        }
#endif
        break;
    case GC_TAG_CHANNEL: // [sync_ptr][buf_ptr]...
        if (h->size >= 56) {
            void *sync = (void *)slots[0];
            void *buf = (void *)slots[1];
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
        break;
    case GC_TAG_ENTITY:
        // The per-instance entity lock in the hidden trailing slot (zero in
        // test mode, where __pluto_rwlock_destroy is a no-op anyway).
        if (h->size >= 8) __pluto_rwlock_destroy(slots[h->size / 8 - 1]);
        break;
    default:
        break;
    }
}

// Sweep one small block: finalize each unmarked object, tag it FREE and push
// it on the block's free list (rebuilt from scratch, including slots that
// were already free); clear the marks of survivors. A block with no
// survivors returns to the pool; one with room joins its class's available
// list. Returns the bytes reclaimed.
#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
static size_t gc_sweep_shared_live = 0;   // shared bytes surviving this global collection
#endif

static size_t gc_sweep_block(GCBlock *b) {
    size_t osz = b->obj_size;
    size_t freed = 0;
    size_t live = 0;
    int containers = 0;
    GCHeader *free_list = NULL;
    for (uint32_t i = 0; i < b->bump; i++) {
        GCHeader *h = (GCHeader *)(b->base + (size_t)i * osz);
        if (h->type_tag != GC_TAG_FREE) {
            if (h->mark) {
                h->mark = 0;
                live++;
                if (gc_is_container_tag(h->type_tag)) containers = 1;
#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
                if (h->next == GC_SHARED_TAG) gc_sweep_shared_live += osz;
                else if (b->owner) b->owner->private_live += osz;
#endif
                continue;
            }
            gc_finalize(h);
            gc_poison(h);
            h->type_tag = GC_TAG_FREE;
            freed += osz;
        }
        h->next = free_list;
        free_list = h;
    }
    if (live == 0) {
        b->kind = GC_BLOCK_POOL;
        b->bump = 0;
        b->free_list = NULL;
        b->has_containers = 0;
        b->owner = NULL;
        b->next = gc_block_pool;
        gc_block_pool = b;
        return freed;
    }
    b->free_list = free_list;
    b->has_containers = (uint8_t)containers;
    GCBlock **list = &gc_class_avail[b->cls];
#if defined(GC_TLAB) && !defined(PLUTO_TEST_MODE)
    if (b->owner) {
        gc_tlh_own(b->owner, b);   // re-register in its owner's block set
        list = &b->owner->avail[b->cls];
    }
#endif
    if (free_list || b->bump < b->nobjs) {
        b->next = *list;
        *list = b;
    } else {
        b->next = NULL;
    }
    return freed;
}

// Sweep large objects: finalize, unmap and free the unmarked ones.
static size_t gc_sweep_large(void) {
    size_t freed = 0;
    for (size_t k = 0; k < gc_large_count;) {
        GCBlock *b = gc_large_blocks[k];
        GCHeader *h = (GCHeader *)b->base;
        if (h->mark) {
            h->mark = 0;
#if defined(GC_TLAB) && !defined(PLUTO_TEST_MODE)
            if (b->owner) gc_tlh_own(b->owner, b);
#endif
#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
            if (h->next == GC_SHARED_TAG) gc_sweep_shared_live += b->obj_size;
            else if (b->owner) b->owner->private_live += b->obj_size;
#endif
            k++;
            continue;
        }
        gc_finalize(h);
        gc_pagemap_set(b->base, b->obj_size >> GC_PAGE_SHIFT, NULL);
        free(b->base);
        freed += b->obj_size;
        b->next = gc_desc_free;
        gc_desc_free = b;
        gc_large_blocks[k] = gc_large_blocks[--gc_large_count];
    }
    return freed;
}

#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
// ── Local collection (--gc tlh) ──────────────────────────────────────────────
//
// A thread collects its own private objects alone: no lock, no handshake,
// nobody else stops. Soundness rests on invariant I. No shared object points
// at a private one, and private objects are reachable from no other thread's
// heap, so the only references to H's private objects are this thread's
// own roots (its stack, registers and thread-locals) and other private
// objects of H. Words on other threads' stacks that look like pointers into
// H are stale or coincidental by the same argument, so they are ignored.
//
// Tracing is confined to H's private objects (gc_local_heap); shared
// objects are neither marked nor traced (they cannot lead back into H). The
// sweep touches only H's blocks: dead private objects are finalized and
// freed, shared ones are left alone (only a global collection frees them).
// Anything that would need gc_mutex — returning emptied blocks to the pool,
// unmapping large objects, adjusting the global byte count — is queued on H
// and handed over at the next report (gc_tlh_report), so a local collection
// never blocks and a global collection simply waits for it to finish.

// Interval table over H's private container backing stores only. A shared
// container must never be marked by a local collection (its mark would
// survive the local sweep and corrupt the next global one).
static void gc_tlh_build_local_intervals(GCThreadHeap *H) {
    gc_data_interval_count = 0;
    for (size_t k = 0; k < H->nblocks; k++) {
        GCBlock *b = H->blocks[k];
        if (!b->has_containers) continue;
        uint32_t n = b->kind == GC_BLOCK_LARGE ? 1 : b->bump;
        size_t osz = b->kind == GC_BLOCK_LARGE ? 0 : b->obj_size;
        for (uint32_t i = 0; i < n; i++) {
            GCHeader *h = (GCHeader *)(b->base + (size_t)i * osz);
            if (gc_is_container_tag(h->type_tag) && h->next != GC_SHARED_TAG) {
                gc_add_container_intervals(h);
            }
        }
    }
    gc_finish_data_intervals();
}

static size_t gc_tlh_local_sweep(GCThreadHeap *H) {
    for (size_t c = 0; c < GC_NUM_CLASSES; c++) H->avail[c] = NULL;
    size_t freed = 0, live_private = 0, kept = 0;
    for (size_t k = 0; k < H->nblocks; k++) {
        GCBlock *b = H->blocks[k];
        if (b->kind == GC_BLOCK_LARGE) {
            GCHeader *h = (GCHeader *)b->base;
            if (h->next != GC_SHARED_TAG) {
                if (!h->mark) {
                    gc_finalize(h);
                    h->type_tag = GC_TAG_FREE;   // invisible to lookups from now on
                    freed += b->obj_size;
                    b->next = H->dead_large;
                    H->dead_large = b;
                    continue;
                }
                h->mark = 0;
                live_private += b->obj_size;
            }
            H->blocks[kept++] = b;
            continue;
        }
        if (b->kind != GC_BLOCK_SMALL) continue;
        size_t osz = b->obj_size, live = 0;
        int containers = 0;
        GCHeader *free_list = NULL;
        for (uint32_t i = 0; i < b->bump; i++) {
            GCHeader *h = (GCHeader *)(b->base + (size_t)i * osz);
            if (h->type_tag != GC_TAG_FREE) {
                if (h->next == GC_SHARED_TAG || h->mark) {
                    if (h->next != GC_SHARED_TAG) {
                        h->mark = 0;
                        live_private += osz;
                    }
                    live++;
                    if (gc_is_container_tag(h->type_tag)) containers = 1;
                    continue;
                }
                gc_finalize(h);
                gc_poison(h);
                h->type_tag = GC_TAG_FREE;
                freed += osz;
            }
            h->next = free_list;
            free_list = h;
        }
        if (live == 0) {
            // Stays owned by H (so no other thread's lookup resolves into it)
            // until the next report returns it to the pool.
            b->kind = GC_BLOCK_POOL;
            b->bump = 0;
            b->free_list = NULL;
            b->has_containers = 0;
            b->next = H->empty;
            H->empty = b;
            continue;
        }
        b->free_list = free_list;
        b->has_containers = (uint8_t)containers;
        H->blocks[kept++] = b;
        if (free_list || b->bump < b->nobjs) {
            b->next = H->avail[b->cls];
            H->avail[b->cls] = b;
        } else {
            b->next = NULL;
        }
    }
    H->nblocks = kept;
    H->private_live = live_private;
    return freed;
}

#if defined(__GNUC__) || defined(__clang__)
__attribute__((noinline))
#endif
static void gc_tlh_local_collect(GCThreadHeap *H) {
    gc_init_env();
    struct timespec t0, tm, t1;
    if (gc_log_enabled) clock_gettime(CLOCK_MONOTONIC, &t0);
    GCMarkCtx *saved = gc_ctx;
    gc_ctx = &H->ctx;
    gc_local_heap = H;
    gc_worklist_count = 0;
    gc_tlh_build_local_intervals(H);

    jmp_buf regs;
    setjmp(regs);
    {
        long *p = (long *)&regs;
        for (size_t i = 0; i < sizeof(regs) / (sizeof(long)); i++) gc_mark_candidate((void *)p[i]);
    }
    {
        volatile long anchor = 0;
        (void)anchor;
        void *lo = (void *)&anchor;
        void *hi = gc_my_slot ? gc_my_slot->stack_hi : gc_stack_bottom;
        if (lo > hi) { void *t = lo; lo = hi; hi = t; }
        lo = (void *)(((size_t)lo) & ~7UL);
        for (long *p = (long *)lo; (void *)p < hi; p++) gc_mark_candidate((void *)*p);
    }
    if (__pluto_current_error) gc_mark_candidate(__pluto_current_error);
    if (__pluto_current_error_type) gc_mark_candidate(__pluto_current_error_type);
    while (gc_worklist_count > 0) gc_trace_object(gc_worklist[--gc_worklist_count]);
    if (gc_log_enabled) clock_gettime(CLOCK_MONOTONIC, &tm);

    size_t freed = gc_tlh_local_sweep(H);
    gc_data_interval_count = 0;   // promotion must not see stale intervals
    gc_worklist_count = 0;
    gc_local_heap = NULL;
    gc_ctx = saved;

    H->local_freed += freed;
    H->local_alloc = 0;
    H->local_threshold = H->private_live * 2;
    if (H->local_threshold < GC_TLH_LOCAL_FLOOR) H->local_threshold = GC_TLH_LOCAL_FLOOR;
    H->local_count++;
    if (gc_log_enabled) {
        clock_gettime(CLOCK_MONOTONIC, &t1);
#define GC_US(a, b) (((b).tv_sec - (a).tv_sec) * 1000000L + ((b).tv_nsec - (a).tv_nsec) / 1000L)
        fprintf(stderr,
                "gc: local #%ld heap=%p live=%zu freed=%zu next_threshold=%zu pause_us=%ld"
                " mark_us=%ld sweep_us=%ld kind=local\n",
                H->local_count, (void *)H, H->private_live, freed, H->local_threshold,
                GC_US(t0, t1), GC_US(t0, tm), GC_US(tm, t1));
#undef GC_US
    }
}

// Task exit (D2's region reclamation). The exit path has already published
// the task's result and error through barriered stores, so by invariant I
// every object still private to H is unreachable: no other thread can
// reach it, and this thread will never run Pluto code again. A sweep with
// no marking therefore frees exactly the dead set, without tracing
// anything; blocks left holding shared objects pass to the shared heap at
// retire. Lock-free, like a local collection.
static void gc_tlh_exit_reclaim(GCThreadHeap *H) {
    gc_init_env();
    struct timespec t0, t1;
    if (gc_log_enabled) clock_gettime(CLOCK_MONOTONIC, &t0);
    size_t freed = gc_tlh_local_sweep(H);   // all marks are clear: every private object dies
    H->local_freed += freed;
    if (gc_log_enabled) {
        clock_gettime(CLOCK_MONOTONIC, &t1);
        fprintf(stderr, "gc: exit heap=%p freed=%zu kept_blocks=%zu pause_us=%ld kind=exit\n",
                (void *)H, freed, H->nblocks,
                (long)((t1.tv_sec - t0.tv_sec) * 1000000L + (t1.tv_nsec - t0.tv_nsec) / 1000L));
    }
}
#endif

#ifdef GC_LAZY_SWEEP
// Lazy sweeping (--gc lazy). A collection only marks: every small block is
// queued on its class's unswept list, and gc_small_alloc sweeps queued blocks
// on demand when its class runs out of room — moving sweep work out of the
// pause and next to the allocation that reuses the memory. Blocks still
// queued when the next collection starts are swept first, so marks never
// leak across cycles. Live bytes come from the bytes marked during the trace.
static size_t gc_marked_bytes = 0;

static size_t gc_slot_bytes(GCHeader *h) {
    size_t total = sizeof(GCHeader) + h->size;
    if (total <= GC_SMALL_MAX) return gc_class_sizes[gc_size_class[(total + 15) >> 4]];
    return (total + GC_PAGE_SIZE - 1) & ~(GC_PAGE_SIZE - 1);
}

static void gc_finish_lazy_sweep(void) {
    for (size_t c = 0; c < GC_NUM_CLASSES; c++) {
        GCBlock *b;
        while ((b = gc_class_unswept[c]) != NULL) {
            gc_class_unswept[c] = b->next;
            gc_sweep_block(b);
        }
    }
}

// Sweep at most max_blocks queued blocks; returns 1 when none remain. Needs
// gc_mutex but not a stopped world: mutators never touch mark bits or dead
// slots.
static int gc_sweep_some(size_t max_blocks) {
    size_t done = 0;
    for (size_t c = 0; c < GC_NUM_CLASSES; c++) {
        GCBlock *b;
        while ((b = gc_class_unswept[c]) != NULL) {
            if (done == max_blocks) return 0;
            gc_class_unswept[c] = b->next;
            gc_sweep_block(b);
            done++;
        }
    }
    return 1;
}
#endif

#ifdef GC_GENERATIONAL
// Tags whose children live (partly) in malloc'd side storage the barrier
// cannot see: old instances are re-traced by every minor collection.
static inline int gc_gen_always_rescan(uint8_t tag) {
    return tag == GC_TAG_ARRAY || tag == GC_TAG_MAP || tag == GC_TAG_SET || tag == GC_TAG_CHANNEL;
}

// Minor-collection remembered set. Runs before root scanning, so at this
// point mark == 1 means exactly "old". Re-traces every old object in a dirty
// block and every old object of an always-rescan tag; tracing pushes the
// young objects they reach (old children are already marked).
static void gc_gen_scan_remembered(void) {
    for (size_t k = 0; k < gc_small_block_count; k++) {
        GCBlock *b = gc_small_blocks[k];
        if (b->kind != GC_BLOCK_SMALL || (!b->dirty && !b->has_containers)) continue;
        for (uint32_t i = 0; i < b->bump; i++) {
            GCHeader *h = (GCHeader *)(b->base + (size_t)i * b->obj_size);
            if (h->type_tag == GC_TAG_FREE || !h->mark) continue;
            if (b->dirty || gc_gen_always_rescan(h->type_tag)) {
                gc_trace_object((char *)h + sizeof(GCHeader));
            }
        }
    }
    for (size_t k = 0; k < gc_large_count; k++) {
        GCBlock *b = gc_large_blocks[k];
        GCHeader *h = (GCHeader *)b->base;
        if (h->mark && (b->dirty || gc_gen_always_rescan(h->type_tag))) {
            gc_trace_object((char *)h + sizeof(GCHeader));
        }
    }
}

// Apply (or remove) read-only protection to every block selected by want(),
// merging address-contiguous blocks into single mprotect calls.
static void gc_gen_protect_pass(int readonly) {
    char *run = NULL;
    size_t run_len = 0;
    for (size_t k = 0; k <= gc_small_block_count; k++) {
        GCBlock *b = k < gc_small_block_count ? gc_small_blocks[k] : NULL;
        int sel = 0;
        if (b && b->kind == GC_BLOCK_SMALL) {
            sel = readonly ? (!b->prot && b->bump > 0) : b->prot;
        }
        if (sel && run && b->base == run + run_len) {
            run_len += GC_BLOCK_SIZE;
        } else {
            if (run) gc_gen_set_prot(run, run_len, readonly);
            run = sel ? b->base : NULL;
            run_len = sel ? GC_BLOCK_SIZE : 0;
        }
        if (sel) b->prot = (uint8_t)readonly;
        if (b) b->dirty = 0;
    }
    for (size_t k = 0; k < gc_large_count; k++) {
        GCBlock *b = gc_large_blocks[k];
        if (readonly ? !b->prot : b->prot) {
            gc_gen_set_prot(b->base, b->obj_size, readonly);
            b->prot = (uint8_t)readonly;
        }
        b->dirty = 0;
    }
}

// Major collection prologue: every mark goes back to 0 (everything young).
static void gc_gen_clear_marks(void) {
    for (size_t k = 0; k < gc_small_block_count; k++) {
        GCBlock *b = gc_small_blocks[k];
        if (b->kind != GC_BLOCK_SMALL) continue;
        for (uint32_t i = 0; i < b->bump; i++) {
            ((GCHeader *)(b->base + (size_t)i * b->obj_size))->mark = 0;
        }
    }
    for (size_t k = 0; k < gc_large_count; k++) {
        ((GCHeader *)gc_large_blocks[k]->base)->mark = 0;
    }
}

// Sticky-mark sweep of one unprotected block: unmarked objects die, marked
// ones (old, or young survivors now promoted) keep mark = 1.
static size_t gc_gen_sweep_block(GCBlock *b) {
    size_t osz = b->obj_size, freed = 0, live = 0;
    int containers = 0;
    GCHeader *free_list = NULL;
    for (uint32_t i = 0; i < b->bump; i++) {
        GCHeader *h = (GCHeader *)(b->base + (size_t)i * osz);
        if (h->type_tag != GC_TAG_FREE) {
            if (h->mark) {
                live++;
                if (gc_is_container_tag(h->type_tag)) containers = 1;
                continue;
            }
            gc_finalize(h);
            gc_poison(h);
            h->type_tag = GC_TAG_FREE;
            freed += osz;
        }
        h->next = free_list;
        free_list = h;
    }
    if (live == 0) {
        b->kind = GC_BLOCK_POOL;
        b->bump = 0;
        b->free_list = NULL;
        b->has_containers = 0;
        b->next = gc_block_pool;
        gc_block_pool = b;
        return freed;
    }
    b->free_list = free_list;
    b->has_containers = (uint8_t)containers;
    return freed;
}
#endif

// Reclaim every unmarked object and clear the marks of the survivors (lazy
// mode: queue the small blocks instead). Returns the bytes reclaimed now.
static size_t gc_sweep(void) {
    for (size_t c = 0; c < GC_NUM_CLASSES; c++) gc_class_avail[c] = NULL;
#if defined(GC_TLAB) && !defined(PLUTO_TEST_MODE)
    // Every heap's lists and block set are rebuilt by the sweep below.
    for (size_t i = 0; i < gc_tlh_heap_count; i++) {
        GCThreadHeap *H = gc_tlh_heaps[i];
        for (size_t c = 0; c < GC_NUM_CLASSES; c++) H->avail[c] = NULL;
        H->nblocks = 0;
#if defined(GC_TLH)
        H->private_live = 0;   // recounted by the sweep
#endif
    }
#endif
#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
    gc_sweep_shared_live = 0;
#endif
    size_t freed = 0;
#ifdef GC_GENERATIONAL
    // Protected blocks hold only old objects (always live in a minor
    // collection) and free slots: nothing to sweep, but their free slots
    // stay available.
    for (size_t k = 0; k < gc_small_block_count; k++) {
        GCBlock *b = gc_small_blocks[k];
        if (b->kind != GC_BLOCK_SMALL) continue;
        if (!b->prot) freed += gc_gen_sweep_block(b);
        if (b->kind == GC_BLOCK_SMALL && (b->free_list || b->bump < b->nobjs)) {
            b->next = gc_class_avail[b->cls];
            gc_class_avail[b->cls] = b;
        }
    }
    for (size_t k = 0; k < gc_large_count;) {
        GCBlock *b = gc_large_blocks[k];
        GCHeader *h = (GCHeader *)b->base;
        if (b->prot || h->mark) { k++; continue; }
        gc_finalize(h);
        gc_pagemap_set(b->base, b->obj_size >> GC_PAGE_SHIFT, NULL);
        free(b->base);
        freed += b->obj_size;
        b->next = gc_desc_free;
        gc_desc_free = b;
        gc_large_blocks[k] = gc_large_blocks[--gc_large_count];
    }
    return freed;
#endif
    for (size_t k = 0; k < gc_small_block_count; k++) {
        GCBlock *b = gc_small_blocks[k];
        if (b->kind != GC_BLOCK_SMALL) continue;
#ifdef GC_LAZY_SWEEP
        b->next = gc_class_unswept[b->cls];
        gc_class_unswept[b->cls] = b;
#else
        freed += gc_sweep_block(b);
#endif
    }
    freed += gc_sweep_large();
    return freed;
}

// ── Garbage Collection ───────────────────────────────────────────────────────

// Account a collection's reclaimed bytes and set the next threshold.
static void gc_after_sweep(size_t freed_bytes) {
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
}

// Mark everything the roots reference: registers, this thread's stack, every
// other registered thread's stack (parked), thread-locals, registered
// globals and pending task handles. Callers have stopped the world.
#if defined(__GNUC__) || defined(__clang__)
__attribute__((noinline))
#endif
static void gc_scan_roots(void) {
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
    if (__pluto_current_error_type) gc_mark_candidate(__pluto_current_error_type);

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

}

#ifdef GC_INCREMENTAL
// ── Incremental marking (--gc incr) ──────────────────────────────────────────
//
// Targets pauses that do not grow with the heap. A cycle marks in short
// stop-the-world steps interleaved with allocation, then sweeps lazily.
//
// Invariant S (snapshot at the beginning): every object reachable when the
// cycle starts, and every object allocated during it, is marked when the
// cycle ends. The start step scans all roots once — stacks are never
// rescanned. Objects allocated while marking are allocated black. While
// marking, __pluto_gc_barrier_mode is 2 and every reference that is
// overwritten in, or removed from, a heap object is logged first
// (PLUTO_GC_STORE / PLUTO_GC_DELETE and the codegen barrier); each step
// shades the logged references. That is enough: a reference the mutator
// holds was reachable at the snapshot (so it is marked, unless the last heap
// path to it was cut before marking reached it — and a cut is logged), or it
// was allocated during the cycle (black). Initializing stores into fresh
// objects overwrite nothing and need no barrier, which is why this design
// suits Pluto's codegen; stores into containers and objects that do
// overwrite are all funneled through the runtime mutators and the field-store
// barrier.
//
// Pacing is by allocation, never by time (deterministic under pluto test):
// every GC_INCR_STEP_BYTES allocated while marking runs one step, which
// traces GC_INCR_WORK_RATIO times as many bytes as were allocated. A step's
// pause is O(logged references + its trace quantum); the start step adds the
// root scan, and the final step finishes the trace and queues the sweep.
// PLUTO_GC_TORTURE=N steps every N allocations with a one-object quantum,
// stretching each cycle over as many mutator interleavings as possible.
static long gc_incr_steps = 0;          // pauses in the current cycle
static long gc_incr_cycle_us = 0;       // their total
static long gc_incr_cycle_max_us = 0;

void __pluto_gc_log_deleted(long old) {
    uintptr_t a = (uintptr_t)old;
    if (a < gc_heap_lo || a >= gc_heap_hi) return;   // not a heap reference
    gc_satb_push(&gc_satb_local, old);
}

static void gc_incr_drain_one(GCSatbBuf *b) {
    for (size_t i = 0; i < b->n; i++) gc_mark_candidate((void *)b->v[i]);
    b->n = 0;
}

// Shade every logged reference. All threads are stopped, so no log grows.
static void gc_incr_drain_logs(void) {
#ifndef PLUTO_TEST_MODE
    for (int i = 0; i < gc_thread_stack_count; i++) {
        GCThreadStack *t = gc_thread_stacks[i];
        if (t->active && t->satb) gc_incr_drain_one(t->satb);
    }
#endif
    gc_incr_drain_one(&gc_satb_local);
    gc_incr_drain_one(&gc_satb_global);
}

// Trace until `budget` bytes of objects have been scanned (0: no limit).
// Returns 1 when the worklist is empty.
static int gc_incr_trace(size_t budget) {
    size_t done = 0;
    while (gc_worklist_count > 0 || gc_cont_count > 0) {
        if (gc_worklist_count > 0) {
            void *o = gc_worklist[--gc_worklist_count];
            gc_trace_object(o);
            done += sizeof(GCHeader) + gc_get_header(o)->size;
        } else {
            GCCont c = gc_conts[--gc_cont_count];
            done += sizeof(GCHeader) + 8 * (size_t)gc_scan_container_chunk(c.obj, c.next);
        }
        if (budget && done >= budget) break;
    }
    return gc_worklist_count == 0 && gc_cont_count == 0;
}

// PLUTO_GC_VERIFY at the end of marking: no marked object may reference an
// unmarked one. Under invariant S such an edge can only come from a missed
// barrier. Slots resolve by exact start, so integers that fall inside an
// object raise no false alarms.
static void gc_incr_check_slot(GCHeader *parent, long word, const char *where) {
    GCHeader *c = gc_lookup((void *)word, 0);
    if (!c || c->mark) return;
    fprintf(stderr, "pluto: PLUTO_GC_VERIFY: incremental marking missed an object: marked %p "
            "(tag %d, size %u) %s holds unmarked %p (tag %d, size %u)\n",
            (void *)(parent + 1), parent->type_tag, parent->size, where,
            (void *)(c + 1), c->type_tag, c->size);
    abort();
}

static void gc_incr_check(void) {
    for (size_t k = 0; k < gc_small_block_count; k++) {
        GCBlock *b = gc_small_blocks[k];
        if (b->kind != GC_BLOCK_SMALL) continue;
        for (uint32_t i = 0; i < b->bump; i++) {
            GCHeader *h = (GCHeader *)(b->base + (size_t)i * b->obj_size);
            if (h->type_tag != GC_TAG_FREE && h->mark) gc_check_children(h, gc_incr_check_slot);
        }
    }
    for (size_t k = 0; k < gc_large_count; k++) {
        GCHeader *h = (GCHeader *)gc_large_blocks[k]->base;
        if (h->mark) gc_check_children(h, gc_incr_check_slot);
    }
}

#define GC_US(a, b) (((b).tv_sec - (a).tv_sec) * 1000000L + ((b).tv_nsec - (a).tv_nsec) / 1000L)

// Marking is complete (worklist and logs empty, world stopped): turn the
// barrier off and queue the lazy sweep.
static void gc_incr_finish(void) {
    if (gc_verify_enabled > 0) gc_incr_check();
    __pluto_gc_barrier_mode = 0;
    gc_incr_marking = 0;
    (void)gc_sweep();   // queues small blocks; large objects are swept now
    size_t freed = gc_bytes_allocated - gc_marked_bytes;
    gc_after_sweep(freed);
    gc_data_interval_count = 0;
    gc_worklist_count = 0;
    gc_cycle_count++;
}

// One stop-the-world step: shade the logs, trace a quantum, and finish the
// cycle if that empties the worklist. `kind` labels the log line.
static void gc_incr_step_at(struct timespec t0, const char *kind) {
    gc_incr_drain_logs();
    size_t budget = gc_torture_every > 0 ? 1 : GC_INCR_WORK_RATIO * GC_INCR_STEP_BYTES;
    int finished = 0;
    if (gc_incr_trace(budget)) {
        do {
            gc_incr_drain_logs();
            gc_incr_trace(0);
        } while (gc_worklist_count > 0 || gc_cont_count > 0);
        gc_incr_finish();
        finished = 1;
    }
    gc_incr_since_step = 0;
    gc_incr_steps++;
    if (gc_log_enabled) {
        struct timespec t1;
        clock_gettime(CLOCK_MONOTONIC, &t1);
        long us = GC_US(t0, t1);
        gc_incr_cycle_us += us;
        if (us > gc_incr_cycle_max_us) gc_incr_cycle_max_us = us;
        if (finished) {
            fprintf(stderr, "gc: #%ld live=%zu next_threshold=%zu pause_us=%ld steps=%ld"
                    " cycle_pause_us=%ld cycle_max_us=%ld kind=finish\n",
                    gc_cycle_count, gc_bytes_allocated, gc_threshold, us, gc_incr_steps,
                    gc_incr_cycle_us, gc_incr_cycle_max_us);
        } else {
            fprintf(stderr, "gc: incr pause_us=%ld left=%zu kind=%s\n", us, gc_worklist_count, kind);
        }
    }
    gc_collecting = 0;
}

// Start a cycle (world stopped): take the root snapshot, turn the barrier
// on, and do a first step. The previous cycle's sweep has normally been
// finished by the allocator already (gc_sweep_some); finishing it here is a
// no-op then.
static void gc_incr_start(void) {
    gc_collecting = 1;
    gc_init_env();
    struct timespec t0;
    if (gc_log_enabled) clock_gettime(CLOCK_MONOTONIC, &t0);
    gc_finish_lazy_sweep();
    gc_marked_bytes = 0;
    gc_build_data_intervals();
    gc_worklist_count = 0;
    gc_scan_roots();
    gc_incr_marking = 1;
    __pluto_gc_barrier_mode = 2;
    gc_incr_steps = 0;
    gc_incr_cycle_us = 0;
    gc_incr_cycle_max_us = 0;
    gc_incr_step_at(t0, "start");
}

static void gc_incr_step(void) {
    gc_collecting = 1;
    struct timespec t0;
    if (gc_log_enabled) clock_gettime(CLOCK_MONOTONIC, &t0);
    gc_incr_step_at(t0, "step");
}
#undef GC_US
#else
void __pluto_gc_log_deleted(long old) { (void)old; }
#endif

void __pluto_gc_collect(void) {
#ifdef GC_INCREMENTAL
    if (gc_incr_marking) {
        // An explicit collection during a cycle completes that cycle.
        gc_collecting = 1;
        do {
            gc_incr_drain_logs();
            gc_incr_trace(0);
        } while (gc_worklist_count > 0 || gc_cont_count > 0);
        gc_incr_finish();
        gc_collecting = 0;
        return;
    }
#endif
    gc_collecting = 1;
    gc_init_env();
    struct timespec gc_t0;
    if (gc_log_enabled) clock_gettime(CLOCK_MONOTONIC, &gc_t0);
#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
    // Everyone is stopped: take over what each heap did without the lock.
    for (size_t i = 0; i < gc_tlh_heap_count; i++) gc_tlh_report(gc_tlh_heaps[i]);
#elif defined(GC_TLAB) && !defined(PLUTO_TEST_MODE)
    for (size_t i = 0; i < gc_tlh_heap_count; i++) {
        gc_bytes_allocated += gc_tlh_heaps[i]->unreported;
        gc_tlh_heaps[i]->unreported = 0;
    }
#endif

#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
    if (gc_verify_enabled > 0) gc_tlh_check_invariant();
#endif
    // Build the data-buffer interval table (objects need no per-cycle index:
    // the page map resolves them directly).
#ifdef GC_GENERATIONAL
    gc_gen_major = gc_gen_barrier_off
                   || gc_bytes_allocated > gc_gen_old_limit + GC_GEN_NURSERY;
    if (gc_gen_major) {
        gc_gen_protect_pass(0);   // unprotect everything
        gc_gen_clear_marks();
    }
#endif
#ifdef GC_LAZY_SWEEP
    // Blocks still queued from the previous cycle carry its marks.
    gc_finish_lazy_sweep();
    gc_marked_bytes = 0;
#endif
    gc_build_data_intervals();
    struct timespec gc_tb;
    if (gc_log_enabled) clock_gettime(CLOCK_MONOTONIC, &gc_tb);

    // Reset worklist
    gc_worklist_count = 0;
#ifdef GC_GENERATIONAL
    if (!gc_gen_major) gc_gen_scan_remembered();
#endif

    gc_scan_roots();

    // 5. Drain worklist (breadth-first trace)
#if defined(GC_PARALLEL_MARK) && !defined(PLUTO_TEST_MODE)
    gc_pm_drain();
#else
    while (gc_worklist_count > 0) {
        void *obj = gc_worklist[--gc_worklist_count];
        gc_trace_object(obj);
    }
#endif

    struct timespec gc_tm;
    if (gc_log_enabled) clock_gettime(CLOCK_MONOTONIC, &gc_tm);

    // ── Sweep phase ───────────────────────────────────────────────────────
    size_t freed_bytes = gc_sweep();
#ifdef GC_LAZY_SWEEP
    // Every unmarked byte counts as reclaimed now; queued blocks are swept
    // (their free lists rebuilt) on demand without further accounting.
    freed_bytes = gc_bytes_allocated - gc_marked_bytes;
#endif

    struct timespec gc_ts;
    if (gc_log_enabled) clock_gettime(CLOCK_MONOTONIC, &gc_ts);

    gc_after_sweep(freed_bytes);
    size_t live = gc_bytes_allocated;
    (void)live;
#if defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
    // Next global collection once the shared heap has grown by as much as
    // survived this one; private garbage is left to local collections.
    gc_shared_growth = 0;
    gc_shared_threshold = gc_sweep_shared_live > GC_TLH_SHARED_FLOOR
                              ? gc_sweep_shared_live : GC_TLH_SHARED_FLOOR;
    for (size_t i = 0; i < gc_tlh_heap_count; i++) {
        GCThreadHeap *H = gc_tlh_heaps[i];
        H->local_alloc = 0;
        H->local_threshold = H->private_live * 2;
        if (H->local_threshold < GC_TLH_LOCAL_FLOOR) H->local_threshold = GC_TLH_LOCAL_FLOOR;
    }
#endif
#ifdef GC_GENERATIONAL
    // Every survivor is old now: protect its block, start a fresh nursery.
    if (!gc_gen_barrier_off) gc_gen_protect_pass(1);
    if (gc_gen_major) {
        gc_gen_major_count++;
        gc_gen_old_limit = live * 2;
        if (gc_gen_old_limit < GC_GEN_MIN_OLD_LIMIT) gc_gen_old_limit = GC_GEN_MIN_OLD_LIMIT;
    } else {
        gc_gen_minor_count++;
    }
    gc_threshold = live + GC_GEN_NURSERY;
#endif

    // Keep the interval tables and worklist allocated across cycles
    // (grow-only) to avoid per-collection malloc/free churn. Only the live
    // counts are reset; the capacities and buffers persist for process life.
    gc_data_interval_count = 0;
    gc_worklist_count = 0;

    gc_cycle_count++;
    if (gc_log_enabled) {
        struct timespec gc_t1;
        clock_gettime(CLOCK_MONOTONIC, &gc_t1);
#define GC_US(a, b) (((b).tv_sec - (a).tv_sec) * 1000000L + ((b).tv_nsec - (a).tv_nsec) / 1000L)
        // Phase split: build = data-buffer interval table, mark = root scan
        // + trace, sweep = reclaim unmarked objects.
        fprintf(stderr,
                "gc: #%ld live=%zu freed=%zu next_threshold=%zu pause_us=%ld"
                " build_us=%ld mark_us=%ld sweep_us=%ld kind=%s\n",
                gc_cycle_count, gc_bytes_allocated, freed_bytes, gc_threshold,
                GC_US(gc_t0, gc_t1), GC_US(gc_t0, gc_tb), GC_US(gc_tb, gc_tm),
                GC_US(gc_tm, gc_ts),
#ifdef GC_GENERATIONAL
                gc_gen_major ? "major" : "minor"
#elif defined(GC_TLH) && !defined(PLUTO_TEST_MODE)
                "global"
#else
                "full"
#endif
                );
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
#ifdef GC_GENERATIONAL
    gc_gen_install_barrier();
#endif
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

GCHeader *__pluto_gc_find_object(void *p) {
    return gc_lookup(p, 0);
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

// Allocating threads mutate block state and the page map, so lookups take
// the heap lock (counted as a safe region while waiting, like every
// gc_mutex wait).
GCHeader *__pluto_gc_find_object(void *p) {
    gc_heap_lock();
    GCHeader *h = gc_lookup(p, 0);
    pthread_mutex_unlock(&gc_mutex);
    return h;
}

size_t __pluto_gc_bytes_allocated(void) {
#if defined(GC_TLAB)
    // Include allocation the thread heaps have not reported yet.
    gc_heap_lock();
    size_t total = gc_bytes_allocated;
    for (size_t i = 0; i < gc_tlh_heap_count; i++) {
        total += gc_tlh_heaps[i]->unreported;
#if defined(GC_TLH)
        size_t f = gc_tlh_heaps[i]->local_freed;
        total = f < total ? total - f : 0;
#endif
    }
    pthread_mutex_unlock(&gc_mutex);
    return total;
#else
    return gc_bytes_allocated;
#endif
}
#endif
