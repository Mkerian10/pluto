//──────────────────────────────────────────────────────────────────────────────
// Pluto Runtime: No-Op Garbage Collector
//
// Arena-style allocator that never collects. Useful for:
// - Benchmarking (isolate GC overhead from program execution)
// - Short-lived programs where GC is unnecessary
// - Reference implementation of the GC API contract
//
// All allocations go through malloc. No collection, no safepoints,
// no thread coordination. The GCHeader linked list is maintained for
// compatibility with runtime code that walks it (e.g., __pluto_gc_get_head).
//──────────────────────────────────────────────────────────────────────────────

#include "builtins.h"

// ── Global State ─────────────────────────────────────────────────────────────

static GCHeader *gc_head = NULL;
static size_t gc_bytes_allocated = 0;

// TLS variables used by threading.c and builtins.c — must be defined by the GC module
__thread void *__pluto_current_error = NULL;
__thread long *__pluto_current_task = NULL;

// ── Core API ─────────────────────────────────────────────────────────────────

void __pluto_gc_init(void *stack_bottom) {
    (void)stack_bottom;
    // Nothing to initialize
}

void __pluto_gc_collect(void) {
    // No-op: never collect
}

void __pluto_safepoint(void) {
    // No-op: no STW coordination needed
}

void *__pluto_alloc(long size) {
    return gc_alloc((size_t)size, GC_TAG_OBJECT, 0);
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

GCHeader *__pluto_gc_get_head(void) {
    return gc_head;
}

// ── Thread/Fiber API Stubs ───────────────────────────────────────────────────

#ifdef PLUTO_TEST_MODE

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
