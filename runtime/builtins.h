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

// ── Thread-Local Storage ─────────────────────────────────────────────────────

// Error handling — thread-local so each thread has its own error state
extern __thread void *__pluto_current_error;

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
void __pluto_gc_collect(void);
void *__pluto_alloc(long size);
void __pluto_safepoint(void);

// Internal GC allocation API (used by runtime, not by generated code)
void *gc_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count);
size_t __pluto_gc_bytes_allocated(void);

#ifdef PLUTO_TEST_MODE
// Fiber stack API for scheduler (test mode only)
void __pluto_gc_register_fiber_stack(char *base, size_t size);
void __pluto_gc_mark_fiber_complete(int fiber_id);
void __pluto_gc_set_current_fiber(int fiber_id);
void __pluto_gc_enable_fiber_scanning(void);
void __pluto_gc_disable_fiber_scanning(void);
// GC collection trigger API
void __pluto_gc_maybe_collect(void);
GCHeader *__pluto_gc_get_head(void);
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
GCHeader *__pluto_gc_get_head(void);
#endif

// ── Forward Declarations ─────────────────────────────────────────────────────

// Error handling
void __pluto_raise_error(void *error_obj);

// String functions (needed by threading for error messages)
void *__pluto_string_new(const char *src, long len);

// String slice functions (needed by codegen for escape materialization)
void *__pluto_string_slice_new(void *backing, long offset, long len);
void *__pluto_string_slice_to_owned(void *s);
void *__pluto_string_escape(void *s);
const char *__pluto_string_to_cstr(void *s);
void __pluto_string_data(void *s, const char **data_out, long *len_out);

// Array functions (needed by GC for marking)
void *__pluto_array_new(long cap);
void __pluto_array_push(void *handle, long value);

// Time functions (needed by threading for select randomization)
long __pluto_time_ns(void);

#endif // PLUTO_BUILTINS_H
