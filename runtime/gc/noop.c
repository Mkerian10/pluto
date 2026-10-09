//──────────────────────────────────────────────────────────────────────────────
// Pluto Runtime: No-Op Garbage Collector
//
// Arena-style allocator that never collects. Useful for:
// - Benchmarking (isolate GC overhead from program execution)
// - Short-lived programs where GC is unnecessary
// - Reference implementation of the GC API contract
//
// All allocations go through malloc. No collection, no safepoints,
// no thread coordination. A GCHeader linked list is kept so
// __pluto_gc_find_object can answer pointer -> object queries.
//──────────────────────────────────────────────────────────────────────────────

#include "builtins.h"

// ── Global State ─────────────────────────────────────────────────────────────

static GCHeader *gc_head = NULL;
static size_t gc_bytes_allocated = 0;
static void noop_index(void *user);

// TLS variables used by threading.c and builtins.c — must be defined by the GC module
__thread void *__pluto_current_error = NULL;
__thread void *__pluto_current_error_type = NULL;
__thread long *__pluto_current_task = NULL;

// ── Core API ─────────────────────────────────────────────────────────────────

void __pluto_gc_init(void *stack_bottom) {
    (void)stack_bottom;
    __pluto_register_exit_check();
}

void __pluto_gc_collect(void) {
    // No-op: never collect
}

void __pluto_gc_enter_safe_region(void) {}
void __pluto_gc_leave_safe_region(void) {}
void __pluto_gc_add_pending_root(void *p) { (void)p; }
void __pluto_gc_remove_pending_root(void *p) { (void)p; }
void __pluto_gc_register_global_root(void *slot) { (void)slot; }
void __pluto_gc_prepare_fork(void) {}
void __pluto_gc_after_fork(int is_child) { (void)is_child; }

void __pluto_safepoint(void) {
    // No-op: no STW coordination needed
}

void *__pluto_alloc(long size) {
    // field_count must match the mark-sweep backend: structural equality
    // and deep copy walk field_count slots (0 would make every pair of
    // same-class objects compare equal).
    if (size == 0) size = 8;
    return gc_alloc((size_t)size, GC_TAG_OBJECT, (uint16_t)(size / 8));
}

void *__pluto_alloc_entity(long size) {
    // Hidden trailing slot: the entity's per-instance method rwlock (see
    // __pluto_entity_rdlock/wrlock in threading.c). Never freed here — the
    // noop backend never collects.
    if (size == 0) size = 8;
    long *ptr = (long *)gc_alloc((size_t)size + 8, GC_TAG_ENTITY, (uint16_t)(size / 8));
    // Both modes: test mode carries a real fiber-aware lock too
    // (rfc-test-harness phase 4 — lock sites are preemption points).
    ptr[size / 8] = __pluto_rwlock_init();
    return ptr;
}

#ifndef PLUTO_TEST_MODE
// Spawned tasks allocate concurrently: serialize the list and the index.
static pthread_mutex_t noop_mutex = PTHREAD_MUTEX_INITIALIZER;
#endif

static void *noop_alloc_unlocked(size_t user_size, uint8_t type_tag, uint16_t field_count);

void *gc_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count) {
#ifndef PLUTO_TEST_MODE
    pthread_mutex_lock(&noop_mutex);
    void *p = noop_alloc_unlocked(user_size, type_tag, field_count);
    pthread_mutex_unlock(&noop_mutex);
    return p;
#else
    return noop_alloc_unlocked(user_size, type_tag, field_count);
#endif
}

static void *noop_alloc_unlocked(size_t user_size, uint8_t type_tag, uint16_t field_count) {
    GCHeader *header = (GCHeader *)malloc(sizeof(GCHeader) + user_size);
    if (!header) {
        fprintf(stderr, "noop gc: out of memory (requested %zu bytes)\n", user_size);
        exit(1);
    }
    header->size = (uint32_t)user_size;
    header->mark = 0;
    header->type_tag = type_tag;
    header->field_count = field_count;
    header->next = gc_head;
    gc_head = header;
    gc_bytes_allocated += user_size + sizeof(GCHeader);
    void *user_data = (void *)(header + 1);
    memset(user_data, 0, user_size);
    noop_index(user_data);
    return user_data;
}

size_t __pluto_gc_bytes_allocated(void) {
    return gc_bytes_allocated;
}

void __pluto_gc_maybe_collect(void) {
    // No-op
}


// Insert-only exact-start hash (objects are never freed here), so pointer
// lookups (deep copy / structural equality) stay O(1) and the noop backend
// remains an honest "no collection" floor. Grown before load exceeds 1/2.
static void **noop_tab = NULL;
static size_t noop_cap = 0, noop_count = 0;

static size_t noop_hash(void *p) {
    uint64_t x = (uint64_t)(uintptr_t)p >> 4;
    x ^= x >> 33;
    x *= 0xff51afd7ed558ccdULL;
    x ^= x >> 33;
    return (size_t)x;
}

static void noop_insert_raw(void **tab, size_t cap, void *p) {
    size_t i = noop_hash(p) & (cap - 1);
    while (tab[i]) i = (i + 1) & (cap - 1);
    tab[i] = p;
}

static void noop_index(void *user) {
    if ((noop_count + 1) * 2 > noop_cap) {
        size_t cap = noop_cap ? noop_cap * 2 : 4096;
        void **t = (void **)calloc(cap, sizeof(void *));
        if (!t) { fprintf(stderr, "noop gc: out of memory\n"); exit(1); }
        for (size_t k = 0; k < noop_cap; k++) if (noop_tab[k]) noop_insert_raw(t, cap, noop_tab[k]);
        free(noop_tab);
        noop_tab = t;
        noop_cap = cap;
    }
    noop_insert_raw(noop_tab, noop_cap, user);
    noop_count++;
}

static GCHeader *noop_find_unlocked(void *p);

GCHeader *__pluto_gc_find_object(void *p) {
#ifndef PLUTO_TEST_MODE
    pthread_mutex_lock(&noop_mutex);
    GCHeader *h = noop_find_unlocked(p);
    pthread_mutex_unlock(&noop_mutex);
    return h;
#else
    return noop_find_unlocked(p);
#endif
}

static GCHeader *noop_find_unlocked(void *p) {
    if (!noop_cap) return NULL;
    size_t i = noop_hash(p) & (noop_cap - 1);
    for (;;) {
        void *e = noop_tab[i];
        if (!e) return NULL;
        if (e == p) return (GCHeader *)((char *)e - sizeof(GCHeader));
        i = (i + 1) & (noop_cap - 1);
    }
}

// ── Thread/Fiber API Stubs ───────────────────────────────────────────────────

#ifdef PLUTO_TEST_MODE

void __pluto_gc_reset_fiber_stacks(void) {}

void __pluto_gc_set_main_stack_floor(void *floor) {
    (void)floor;
}

void __pluto_gc_set_scheduler_region(void *base, size_t size) {
    (void)base; (void)size;
}

void __pluto_gc_register_fiber_stack(char *base, size_t size) {
    (void)base; (void)size;
}

void __pluto_gc_mark_fiber_complete(int fiber_id) {
    (void)fiber_id;
}

void __pluto_gc_set_current_fiber(int fiber_id) {
    (void)fiber_id;
}

void __pluto_gc_enable_fiber_scanning(void) {}
void __pluto_gc_disable_fiber_scanning(void) {}

#else

void __pluto_gc_register_thread_stack(void *stack_lo, void *stack_hi) {
    (void)stack_lo; (void)stack_hi;
}

void __pluto_gc_deregister_thread_stack(void) {}

int __pluto_gc_active_tasks(void) {
    return 0;
}

void __pluto_gc_task_start(void) {}
void __pluto_gc_task_end(void) {}

int __pluto_gc_check_safepoint(void) {
    return 0;
}

#endif

// Heaps are not private in this backend: nothing ever needs promotion.
void __pluto_gc_promote_store(long value) { (void)value; }
