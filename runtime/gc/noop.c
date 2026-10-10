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

void *gc_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count) {
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
    return user_data;
}

size_t __pluto_gc_bytes_allocated(void) {
    return gc_bytes_allocated;
}

void __pluto_gc_maybe_collect(void) {
    // No-op
}


// The noop backend keeps no index; exact-start linear scan (benchmark-only).
GCHeader *__pluto_gc_find_object(void *p) {
    for (GCHeader *h = gc_head; h; h = h->next) {
        if ((void *)(h + 1) == p) return h;
    }
    return NULL;
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

// Green-task context registry (#369): no-ops under the noop backend (nothing
// is ever collected). The green scheduler calls these unconditionally, so they
// must link in every GC backend.
void *__pluto_gc_register_green_context(void *stack_top, void *live_sp) {
    (void)stack_top; (void)live_sp; return (void *)0;
}
void __pluto_gc_green_set_live_sp(void *handle, void *live_sp) {
    (void)handle; (void)live_sp;
}
void __pluto_gc_unregister_green_context(void *handle) { (void)handle; }

int __pluto_gc_active_tasks(void) {
    return 0;
}

void __pluto_gc_task_start(void) {}
void __pluto_gc_task_end(void) {}

int __pluto_gc_check_safepoint(void) {
    return 0;
}

#endif
