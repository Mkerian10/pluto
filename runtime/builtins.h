// ═══════════════════════════════════════════════════════════════════════════
// Pluto Runtime — Shared Declarations
// ═══════════════════════════════════════════════════════════════════════════
//
// This header provides shared declarations for the Pluto runtime, which is
// split into three modules:
//
//   • gc.c         — Garbage collector (mark & sweep, STW coordination)
//   • threading.c  — Concurrency (tasks, channels, select, fiber scheduler)
//   • builtins.c   — Core runtime (strings, arrays, I/O, maps, sets)
//
// MODULE DEPENDENCIES:
//   threading.c ──► gc.c        (allocation, thread stack registration)
//   builtins.c  ──► gc.c        (allocation for runtime objects)
//   gc.c        ──► (no deps)   (foundational layer)
//
// PUBLIC API:
//   Functions prefixed with __pluto_ are called by generated code or external
//   runtime modules. Functions without this prefix (e.g., gc_alloc) are internal
//   to the runtime and should not be called by generated code.
//
// ═══════════════════════════════════════════════════════════════════════════

#ifndef PLUTO_BUILTINS_H
#define PLUTO_BUILTINS_H

#define _XOPEN_SOURCE 700
#define _GNU_SOURCE
#ifdef __APPLE__
#define _DARWIN_C_SOURCE
#endif

#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <ctype.h>
#include <setjmp.h>
#include <time.h>
#include <sys/time.h>
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
// Kernel-assisted file→socket relay (issue #373). Linux declares
// sendfile(2) in <sys/sendfile.h>. Darwin's declaration lives in
// <sys/socket.h> but is hidden under `#if !defined(_POSIX_C_SOURCE)` —
// with no _DARWIN_C_SOURCE escape on that one guard — and this runtime
// compiles with _XOPEN_SOURCE 700, so declare it ourselves (struct
// sf_hdtr IS visible: its guard honors _DARWIN_C_SOURCE).
#ifdef __APPLE__
#include <sys/uio.h>
int sendfile(int, int, off_t, off_t *, struct sf_hdtr *, int);
#elif defined(__linux__)
#include <sys/sendfile.h>
#endif
#ifndef PLUTO_TEST_MODE
#include <pthread.h>
#include <stdatomic.h>
#endif
#ifdef PLUTO_TEST_MODE
#include <ucontext.h>
#endif

// ── GC Tags ──────────────────────────────────────────────────────────────────

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
#define GC_TAG_STRING_SLICE 10 // [backing_ptr][offset][len]; lightweight view into owned string
#define GC_TAG_ENTITY 11  // object (entity) instance: identity semantics — never deep-copied, never structurally compared (rfc-objects.md)
#define GC_TAG_HANDLE 12  // foreign-entity handle stub: [home_str][type_str][id] (rfc-objects.md phase 2)

// ── Thread-Local Storage ─────────────────────────────────────────────────────

// Error handling — thread-local so each thread has its own error state
extern __thread void *__pluto_current_error;
extern __thread void *__pluto_current_error_type;

// Task handle — thread-local pointer to current task (NULL on main thread)
extern __thread long *__pluto_current_task;

// ── GC Header ────────────────────────────────────────────────────────────────

typedef struct GCHeader {
    struct GCHeader *next;    // 8B: linked list of all GC objects
    uint32_t size;            // 4B: user data size in bytes
    uint8_t  mark;            // 1B: 0=unmarked, 1=marked
    uint8_t  type_tag;        // 1B: object kind
    uint16_t field_count;     // 2B: number of 8-byte slots to scan
} GCHeader;

// ── Write barriers ────────────────────────────────────────────────────────────
//
// __pluto_gc_barrier_mode, defined by every GC backend, is a bit set
// selecting which barriers are live:
//
//   bit 0 (1)  promotion (thread-local heaps). A live object whose header
//              `next` word is GC_SHARED_TAG is shared between threads;
//              storing a pointer into it must first promote the stored value
//              (__pluto_gc_promote_store).
//   bit 1 (2)  snapshot-at-the-beginning logging (incremental marking, only
//              while a cycle is marking). A reference that is overwritten in,
//              or removed from, a heap object is logged
//              (__pluto_gc_log_deleted) so marking still finds everything
//              that was reachable when the cycle began.
//
// PLUTO_GC_STORE(obj, old, value) guards a store of `value` into `obj` over
// `old` (0 for a fresh slot); PLUTO_GC_DELETE(old) guards a removal. Codegen
// emits the same logic before non-scalar field stores. Neither macro may be
// separated from its store by anything that can reach a safepoint.
#define GC_SHARED_TAG ((GCHeader *)(uintptr_t)1)
extern int __pluto_gc_barrier_mode;
void __pluto_gc_promote_store(long value);
void __pluto_gc_log_deleted(long old);
#define PLUTO_GC_STORE(obj, old, value)                                           \
    do {                                                                          \
        int m_ = __pluto_gc_barrier_mode;                                         \
        if (__builtin_expect(m_ != 0, 0)) {                                       \
            if ((m_ & 1)                                                          \
                && ((GCHeader *)((char *)(obj) - sizeof(GCHeader)))->next         \
                   == GC_SHARED_TAG)                                              \
                __pluto_gc_promote_store((long)(value));                          \
            if ((m_ & 2) && (old) != 0) __pluto_gc_log_deleted((long)(old));      \
        }                                                                         \
    } while (0)
#define PLUTO_GC_DELETE(old)                                                      \
    do {                                                                          \
        if (__builtin_expect(__pluto_gc_barrier_mode & 2, 0))                     \
            __pluto_gc_log_deleted((long)(old));                                  \
    } while (0)

// ── Channel Sync (Production Mode Only) ──────────────────────────────────────

#ifndef PLUTO_TEST_MODE
typedef struct {
    pthread_mutex_t mutex;
    pthread_cond_t not_empty;
    pthread_cond_t not_full;
} ChannelSync;
#endif

// ── GC Public API (implemented in gc.c) ──────────────────────────────────────

void __pluto_gc_init(void *stack_bottom);
// Registers the atexit handler that fails the process if an error escapes
// main unhandled (defined in builtins.c, called from __pluto_gc_init).
void __pluto_register_exit_check(void);
void __pluto_gc_collect(void);
void *__pluto_alloc(long size);
void __pluto_safepoint(void);

// Safe (native) regions: bracket code that blocks without touching the GC
// heap (cond waits, blocking syscalls). A thread inside a safe region counts
// as stopped for stop-the-world; leave parks until an in-progress collection
// finishes. No-ops in test mode and the noop backend.
void __pluto_gc_enter_safe_region(void);
void __pluto_gc_leave_safe_region(void);

// Pending-task roots: keep a spawned task handle alive between spawn and the
// new thread registering its stack. No-ops in test mode and the noop backend.
void __pluto_gc_add_pending_root(void *p);
void __pluto_gc_remove_pending_root(void *p);

// Global roots: register the ADDRESS of a module global that holds a GC
// reference (e.g. a DI singleton slot). The collector re-reads each slot at
// every cycle and marks what it currently points to. Called by generated
// startup code; a no-op in the noop backend (which never collects).
void __pluto_gc_register_global_root(void *slot);

// Fork support: hold the allocator lock across fork(); in the child, reset
// GC coordination state (ghost threads from the parent must not be waited on).
void __pluto_gc_prepare_fork(void);
void __pluto_gc_after_fork(int is_child);

// Internal GC allocation API (used by runtime, not by generated code)
void *gc_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count);
size_t __pluto_gc_bytes_allocated(void);
// Resolve a pointer to the GC object whose user data STARTS at it, or NULL
// (non-pointers, interior pointers, static data). O(1) in the mark-sweep
// backend. Used by deep copy and structural equality.
GCHeader *__pluto_gc_find_object(void *p);

// ── Rwlocks & per-instance entity locks (threading.c) ────────────────────────

// Raw rwlock API (per-type locks for synchronized singletons/served classes,
// emitted by codegen). No-ops in test mode (single-threaded fiber scheduler).
long __pluto_rwlock_init(void);
void __pluto_rwlock_rdlock(long lock_ptr);
void __pluto_rwlock_wrlock(long lock_ptr);
void __pluto_rwlock_unlock(long lock_ptr);
void __pluto_rwlock_destroy(long lock_ptr);

// Per-instance entity locks (rfc-objects.md): every entity allocation carries
// one hidden trailing slot (see __pluto_alloc_entity in the GC backends)
// holding an rwlock pointer. Method calls on an entity serialize through THAT
// lock, so distinct instances of one object type run concurrently. No-ops in
// test mode.
void __pluto_entity_rdlock(void *entity);
void __pluto_entity_wrlock(void *entity);
void __pluto_entity_unlock(void *entity);

// Test bookkeeping (builtins.c): current test display name for failure
// repro blocks, and the --test filter flag (PLUTO_TEST_FILTER).
const char *__pluto_test_current_name(void);
int __pluto_test_should_skip(void);

#ifdef PLUTO_TEST_MODE
// Failure repro block (threading.c): strategy/seed/iteration plus the
// recorded schedule token — printed on every failure path.
void __pluto_test_print_repro(void);
// Timed yield (threading.c): the degenerate timed wait backing
// std.time.sleep in test mode — a yield point with an enabled timeout
// choice; resumes when the scheduler picks it (duration erased).
void __pluto_test_timed_yield(void);
// Logical clock (threading.c): the only time source user code observes in
// test mode. +1ms per scheduler dispatch, +1us per query; reset per run.
long __pluto_test_logical_ns(void);
// Per-run std.random reseed (builtins.c), called by the scheduler at the
// start of every schedule run with that run's seed.
void __pluto_rng_reset_test(unsigned long long seed);
// Fiber stack API for scheduler (test mode only). The registry is run-scoped:
// reset must be called at the start of every schedule run, before fiber 0
// registers (stacks are freed per run; stale entries poison the scanner).
void __pluto_gc_reset_fiber_stacks(void);
void __pluto_gc_set_main_stack_floor(void *floor);
void __pluto_gc_set_scheduler_region(void *base, size_t size);
void __pluto_gc_register_fiber_stack(char *base, size_t size);
void __pluto_gc_mark_fiber_complete(int fiber_id);
void __pluto_gc_set_current_fiber(int fiber_id);
void __pluto_gc_enable_fiber_scanning(void);
void __pluto_gc_disable_fiber_scanning(void);
// GC collection trigger API
void __pluto_gc_maybe_collect(void);
#else
// Thread stack API for spawned tasks (production mode only)
void __pluto_gc_register_thread_stack(void *stack_lo, void *stack_hi);
void __pluto_gc_deregister_thread_stack(void);
int __pluto_gc_active_tasks(void);
void __pluto_gc_task_start(void);
void __pluto_gc_task_end(void);
int __pluto_gc_check_safepoint(void);
// GC collection trigger API
void __pluto_gc_maybe_collect(void);
#endif

// ── Forward Declarations ─────────────────────────────────────────────────────

// Error handling
void __pluto_raise_error(void *error_obj);
void __pluto_set_error_type(void *type_str);

// String functions (needed by threading for error messages)
void *__pluto_string_new(const char *src, long len);

// String slice functions (needed by codegen for escape materialization)
void *__pluto_string_slice_new(void *backing, long offset, long len);
void *__pluto_string_slice_to_owned(void *s);
void *__pluto_string_escape(void *s);
const char *__pluto_string_to_cstr(void *s);
void __pluto_string_data(void *s, const char **data_out, long *len_out);
int __pluto_string_eq(void *a, void *b);
long __pluto_socket_close(long fd);

// Coverage functions
void __pluto_coverage_init(long num_points, void *path_str);
void __pluto_coverage_hit(long point_id);

// Array functions (needed by GC for marking)
void *__pluto_array_new(long cap);
void __pluto_array_push(void *handle, long value);

// Time functions (needed by threading for select randomization)
long __pluto_time_ns(void);

#endif // PLUTO_BUILTINS_H
