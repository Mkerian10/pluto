#define _GNU_SOURCE
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
#include <math.h>
#include <pthread.h>
#include <stdatomic.h>

// ── GC Infrastructure ─────────────────────────────────────────────────────────

// Type tags for GC objects
#define GC_TAG_OBJECT 0   // class, enum, closure, error, DI singleton
#define GC_TAG_STRING 1   // no child pointers
#define GC_TAG_ARRAY  2   // handle [len][cap][data_ptr]; data buffer freed on sweep
#define GC_TAG_TRAIT  3   // [data_ptr][vtable_ptr]; trace data_ptr only
#define GC_TAG_MAP   4   // [count][cap][keys_ptr][vals_ptr][meta_ptr]
#define GC_TAG_SET   5   // [count][cap][keys_ptr][meta_ptr]
#define GC_TAG_JSON  6   // JsonNode — recursive tree structure
#define GC_TAG_TASK  7   // [closure][result][error][done][sync_ptr]
#define GC_TAG_BYTES 8   // [len][cap][data_ptr]; 1 byte per element

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

// JSON node types
#define JSON_NULL    0
#define JSON_BOOL    1
#define JSON_INT     2
#define JSON_FLOAT   3
#define JSON_STRING  4
#define JSON_ARRAY   5
#define JSON_OBJECT  6

typedef struct JsonNode {
    int type;
    union {
        int bool_val;
        int64_t int_val;
        double float_val;
        char *string_val;
        struct { struct JsonNode **items; int len; int cap; } array;
        struct { char **keys; struct JsonNode **vals; int len; int cap; } object;
    };
} JsonNode;

// Forward declarations
void __pluto_gc_collect(void);
void *__pluto_array_new(long cap);
void __pluto_array_push(void *handle, long value);
static void json_free_tree(JsonNode *node);

// Error handling — thread-local so each thread has its own error state
__thread void *__pluto_current_error = NULL;

// GC thread safety
static pthread_mutex_t gc_mutex = PTHREAD_MUTEX_INITIALIZER;
static atomic_int __pluto_active_tasks = 0;
#define PLUTO_MAX_HEAP_BYTES (1024L * 1024L * 1024L)  // 1 GB

static inline GCHeader *gc_get_header(void *user_ptr) {
    return (GCHeader *)((char *)user_ptr - sizeof(GCHeader));
}

static void *gc_alloc(size_t user_size, uint8_t type_tag, uint16_t field_count) {
    pthread_mutex_lock(&gc_mutex);
    if (gc_stack_bottom && !gc_collecting
        && gc_bytes_allocated + user_size + sizeof(GCHeader) > gc_threshold
        && atomic_load(&__pluto_active_tasks) == 0) {
        __pluto_gc_collect();
    }
    // Heap ceiling guardrail when GC is suppressed
    if (atomic_load(&__pluto_active_tasks) > 0
        && gc_bytes_allocated + user_size + sizeof(GCHeader) > PLUTO_MAX_HEAP_BYTES) {
        pthread_mutex_unlock(&gc_mutex);
        fprintf(stderr, "pluto: heap exceeded %ld bytes while GC suppressed by active tasks\n",
                (long)PLUTO_MAX_HEAP_BYTES);
        exit(1);
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

    // 3. Scan stack (direction-agnostic)
    {
        void *stack_top;
        // Get current stack pointer
        volatile long anchor = 0;
        (void)anchor;
        stack_top = (void *)&anchor;

        void *lo = stack_top < gc_stack_bottom ? stack_top : gc_stack_bottom;
        void *hi = stack_top < gc_stack_bottom ? gc_stack_bottom : stack_top;
        // Align to 8-byte boundary
        lo = (void *)(((size_t)lo) & ~7UL);
        for (long *p = (long *)lo; (void *)p < hi; p++) {
            gc_mark_candidate((void *)*p);
        }
    }

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
            // Free JSON tree
            if (h->type_tag == GC_TAG_JSON) {
                long *slots = (long *)((char *)h + sizeof(GCHeader));
                JsonNode *root = (JsonNode *)slots[0];
                if (root) json_free_tree(root);
            }
            // Free task sync resources
            if (h->type_tag == GC_TAG_TASK && h->size >= 40) {
                long *slots = (long *)((char *)h + sizeof(GCHeader));
                void *sync = (void *)slots[4];
                if (sync) {
                    pthread_mutex_destroy((pthread_mutex_t *)sync);
                    pthread_cond_destroy((pthread_cond_t *)((char *)sync + sizeof(pthread_mutex_t)));
                    free(sync);
                }
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

void *__pluto_string_concat(void *a, void *b) {
    long len_a = *(long *)a;
    long len_b = *(long *)b;
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

// ── JSON runtime ──────────────────────────────────────────────────────────────
// JsonNode is a malloc'd tree. The GC holds a handle [node_ptr] with GC_TAG_JSON.
// When the GC handle is swept, json_free_tree recursively frees the tree.

static JsonNode *json_new_node(int type) {
    JsonNode *n = (JsonNode *)calloc(1, sizeof(JsonNode));
    if (!n) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    n->type = type;
    return n;
}

static void json_free_tree(JsonNode *node) {
    if (!node) return;
    switch (node->type) {
    case JSON_STRING:
        free(node->string_val);
        break;
    case JSON_ARRAY:
        for (int i = 0; i < node->array.len; i++)
            json_free_tree(node->array.items[i]);
        free(node->array.items);
        break;
    case JSON_OBJECT:
        for (int i = 0; i < node->object.len; i++) {
            free(node->object.keys[i]);
            json_free_tree(node->object.vals[i]);
        }
        free(node->object.keys);
        free(node->object.vals);
        break;
    }
    free(node);
}

// Wrap a JsonNode* in a GC handle (1 field = 8 bytes)
static void *json_wrap(JsonNode *node) {
    long *handle = (long *)gc_alloc(8, GC_TAG_JSON, 1);
    handle[0] = (long)node;
    return handle;
}

static JsonNode *json_unwrap(void *handle) {
    return (JsonNode *)(((long *)handle)[0]);
}

// ── JSON constructors ─────────────────────────────────────────────────────────

void *__pluto_json_new_null(void) {
    return json_wrap(json_new_node(JSON_NULL));
}

void *__pluto_json_new_bool(long v) {
    JsonNode *n = json_new_node(JSON_BOOL);
    n->bool_val = v ? 1 : 0;
    return json_wrap(n);
}

void *__pluto_json_new_int(long v) {
    JsonNode *n = json_new_node(JSON_INT);
    n->int_val = v;
    return json_wrap(n);
}

void *__pluto_json_new_float(double v) {
    JsonNode *n = json_new_node(JSON_FLOAT);
    n->float_val = v;
    return json_wrap(n);
}

void *__pluto_json_new_string(void *pluto_str) {
    long slen = *(long *)pluto_str;
    const char *data = (const char *)pluto_str + 8;
    JsonNode *n = json_new_node(JSON_STRING);
    n->string_val = (char *)malloc(slen + 1);
    memcpy(n->string_val, data, slen);
    n->string_val[slen] = '\0';
    return json_wrap(n);
}

void *__pluto_json_new_array(void) {
    JsonNode *n = json_new_node(JSON_ARRAY);
    n->array.cap = 4;
    n->array.len = 0;
    n->array.items = (JsonNode **)calloc(4, sizeof(JsonNode *));
    return json_wrap(n);
}

void *__pluto_json_new_object(void) {
    JsonNode *n = json_new_node(JSON_OBJECT);
    n->object.cap = 4;
    n->object.len = 0;
    n->object.keys = (char **)calloc(4, sizeof(char *));
    n->object.vals = (JsonNode **)calloc(4, sizeof(JsonNode *));
    return json_wrap(n);
}

// ── JSON type queries ─────────────────────────────────────────────────────────

long __pluto_json_is_null(void *handle)   { return json_unwrap(handle)->type == JSON_NULL   ? 1 : 0; }
long __pluto_json_is_bool(void *handle)   { return json_unwrap(handle)->type == JSON_BOOL   ? 1 : 0; }
long __pluto_json_is_int(void *handle)    { return json_unwrap(handle)->type == JSON_INT    ? 1 : 0; }
long __pluto_json_is_float(void *handle)  { return json_unwrap(handle)->type == JSON_FLOAT  ? 1 : 0; }
long __pluto_json_is_string(void *handle) { return json_unwrap(handle)->type == JSON_STRING ? 1 : 0; }
long __pluto_json_is_array(void *handle)  { return json_unwrap(handle)->type == JSON_ARRAY  ? 1 : 0; }
long __pluto_json_is_object(void *handle) { return json_unwrap(handle)->type == JSON_OBJECT ? 1 : 0; }

// ── JSON accessors ────────────────────────────────────────────────────────────

long __pluto_json_get_bool(void *handle) {
    JsonNode *n = json_unwrap(handle);
    if (n->type != JSON_BOOL) { fprintf(stderr, "pluto: json: not a bool\n"); exit(1); }
    return n->bool_val ? 1 : 0;
}

long __pluto_json_get_int(void *handle) {
    JsonNode *n = json_unwrap(handle);
    if (n->type == JSON_INT) return n->int_val;
    if (n->type == JSON_FLOAT) return (long)n->float_val;
    fprintf(stderr, "pluto: json: not a number\n"); exit(1);
}

double __pluto_json_get_float(void *handle) {
    JsonNode *n = json_unwrap(handle);
    if (n->type == JSON_FLOAT) return n->float_val;
    if (n->type == JSON_INT) return (double)n->int_val;
    fprintf(stderr, "pluto: json: not a number\n"); exit(1);
}

void *__pluto_json_get_string(void *handle) {
    JsonNode *n = json_unwrap(handle);
    if (n->type != JSON_STRING) { fprintf(stderr, "pluto: json: not a string\n"); exit(1); }
    return __pluto_string_new(n->string_val, (long)strlen(n->string_val));
}

void *__pluto_json_get_field(void *handle, void *key_str) {
    JsonNode *n = json_unwrap(handle);
    if (n->type != JSON_OBJECT) { fprintf(stderr, "pluto: json: not an object\n"); exit(1); }
    long klen = *(long *)key_str;
    const char *kdata = (const char *)key_str + 8;
    for (int i = 0; i < n->object.len; i++) {
        if ((long)strlen(n->object.keys[i]) == klen && memcmp(n->object.keys[i], kdata, klen) == 0) {
            return json_wrap(n->object.vals[i]);
        }
    }
    // Return null json if key not found
    return json_wrap(json_new_node(JSON_NULL));
}

void *__pluto_json_get_index(void *handle, long index) {
    JsonNode *n = json_unwrap(handle);
    if (n->type != JSON_ARRAY) { fprintf(stderr, "pluto: json: not an array\n"); exit(1); }
    if (index < 0 || index >= n->array.len) {
        fprintf(stderr, "pluto: json: array index %ld out of bounds (len %d)\n", index, n->array.len);
        exit(1);
    }
    return json_wrap(n->array.items[index]);
}

long __pluto_json_len(void *handle) {
    JsonNode *n = json_unwrap(handle);
    if (n->type == JSON_ARRAY) return n->array.len;
    if (n->type == JSON_OBJECT) return n->object.len;
    return 0;
}

// ── JSON mutators ─────────────────────────────────────────────────────────────

void __pluto_json_array_push(void *handle, void *item_handle) {
    JsonNode *arr = json_unwrap(handle);
    JsonNode *item = json_unwrap(item_handle);
    if (arr->type != JSON_ARRAY) { fprintf(stderr, "pluto: json: not an array\n"); exit(1); }
    if (arr->array.len >= arr->array.cap) {
        arr->array.cap *= 2;
        arr->array.items = (JsonNode **)realloc(arr->array.items, arr->array.cap * sizeof(JsonNode *));
    }
    arr->array.items[arr->array.len++] = item;
}

void __pluto_json_object_set(void *handle, void *key_str, void *val_handle) {
    JsonNode *obj = json_unwrap(handle);
    JsonNode *val = json_unwrap(val_handle);
    if (obj->type != JSON_OBJECT) { fprintf(stderr, "pluto: json: not an object\n"); exit(1); }
    long klen = *(long *)key_str;
    const char *kdata = (const char *)key_str + 8;
    // Check if key already exists
    for (int i = 0; i < obj->object.len; i++) {
        if ((long)strlen(obj->object.keys[i]) == klen && memcmp(obj->object.keys[i], kdata, klen) == 0) {
            json_free_tree(obj->object.vals[i]);
            obj->object.vals[i] = val;
            return;
        }
    }
    // Add new key
    if (obj->object.len >= obj->object.cap) {
        obj->object.cap *= 2;
        obj->object.keys = (char **)realloc(obj->object.keys, obj->object.cap * sizeof(char *));
        obj->object.vals = (JsonNode **)realloc(obj->object.vals, obj->object.cap * sizeof(JsonNode *));
    }
    char *key_copy = (char *)malloc(klen + 1);
    memcpy(key_copy, kdata, klen);
    key_copy[klen] = '\0';
    obj->object.keys[obj->object.len] = key_copy;
    obj->object.vals[obj->object.len] = val;
    obj->object.len++;
}

// ── JSON parser ───────────────────────────────────────────────────────────────

typedef struct {
    const char *src;
    int pos;
    int len;
} JsonParser;

static void jp_skip_ws(JsonParser *p) {
    while (p->pos < p->len) {
        char c = p->src[p->pos];
        if (c == ' ' || c == '\t' || c == '\n' || c == '\r') p->pos++;
        else break;
    }
}

static char jp_peek(JsonParser *p) {
    jp_skip_ws(p);
    if (p->pos >= p->len) return '\0';
    return p->src[p->pos];
}

static char jp_next(JsonParser *p) {
    jp_skip_ws(p);
    if (p->pos >= p->len) return '\0';
    return p->src[p->pos++];
}

static int jp_match(JsonParser *p, const char *word) {
    int wlen = (int)strlen(word);
    if (p->pos + wlen > p->len) return 0;
    if (memcmp(p->src + p->pos, word, wlen) != 0) return 0;
    p->pos += wlen;
    return 1;
}

static JsonNode *jp_parse_value(JsonParser *p);

static JsonNode *jp_parse_string_node(JsonParser *p) {
    if (p->src[p->pos] != '"') return NULL;
    p->pos++; // skip opening "
    int start = p->pos;
    // Estimate: string with escapes
    int cap = 64;
    char *buf = (char *)malloc(cap);
    int blen = 0;
    while (p->pos < p->len && p->src[p->pos] != '"') {
        char c = p->src[p->pos];
        if (c == '\\') {
            p->pos++;
            if (p->pos >= p->len) { free(buf); return NULL; }
            char esc = p->src[p->pos];
            switch (esc) {
            case '"': c = '"'; break;
            case '\\': c = '\\'; break;
            case '/': c = '/'; break;
            case 'b': c = '\b'; break;
            case 'f': c = '\f'; break;
            case 'n': c = '\n'; break;
            case 'r': c = '\r'; break;
            case 't': c = '\t'; break;
            case 'u': {
                // Parse 4 hex digits, emit UTF-8 (BMP only for simplicity)
                if (p->pos + 4 >= p->len) { free(buf); return NULL; }
                unsigned int cp = 0;
                for (int i = 0; i < 4; i++) {
                    p->pos++;
                    char h = p->src[p->pos];
                    cp <<= 4;
                    if (h >= '0' && h <= '9') cp |= h - '0';
                    else if (h >= 'a' && h <= 'f') cp |= 10 + h - 'a';
                    else if (h >= 'A' && h <= 'F') cp |= 10 + h - 'A';
                    else { free(buf); return NULL; }
                }
                // UTF-8 encode
                if (blen + 4 >= cap) { cap *= 2; buf = (char *)realloc(buf, cap); }
                if (cp < 0x80) {
                    buf[blen++] = (char)cp;
                } else if (cp < 0x800) {
                    buf[blen++] = (char)(0xC0 | (cp >> 6));
                    buf[blen++] = (char)(0x80 | (cp & 0x3F));
                } else {
                    buf[blen++] = (char)(0xE0 | (cp >> 12));
                    buf[blen++] = (char)(0x80 | ((cp >> 6) & 0x3F));
                    buf[blen++] = (char)(0x80 | (cp & 0x3F));
                }
                p->pos++;
                continue;
            }
            default: c = esc; break;
            }
        }
        if (blen + 1 >= cap) { cap *= 2; buf = (char *)realloc(buf, cap); }
        buf[blen++] = c;
        p->pos++;
    }
    if (p->pos >= p->len) { free(buf); return NULL; } // unterminated string
    p->pos++; // skip closing "
    buf[blen] = '\0';
    JsonNode *n = json_new_node(JSON_STRING);
    n->string_val = (char *)realloc(buf, blen + 1);
    (void)start;
    return n;
}

static JsonNode *jp_parse_number(JsonParser *p) {
    int start = p->pos;
    int is_float = 0;
    if (p->src[p->pos] == '-') p->pos++;
    while (p->pos < p->len && p->src[p->pos] >= '0' && p->src[p->pos] <= '9') p->pos++;
    if (p->pos < p->len && p->src[p->pos] == '.') {
        is_float = 1;
        p->pos++;
        while (p->pos < p->len && p->src[p->pos] >= '0' && p->src[p->pos] <= '9') p->pos++;
    }
    if (p->pos < p->len && (p->src[p->pos] == 'e' || p->src[p->pos] == 'E')) {
        is_float = 1;
        p->pos++;
        if (p->pos < p->len && (p->src[p->pos] == '+' || p->src[p->pos] == '-')) p->pos++;
        while (p->pos < p->len && p->src[p->pos] >= '0' && p->src[p->pos] <= '9') p->pos++;
    }
    int numlen = p->pos - start;
    char *tmp = (char *)malloc(numlen + 1);
    memcpy(tmp, p->src + start, numlen);
    tmp[numlen] = '\0';
    JsonNode *n;
    if (is_float) {
        n = json_new_node(JSON_FLOAT);
        n->float_val = strtod(tmp, NULL);
    } else {
        long v = strtol(tmp, NULL, 10);
        n = json_new_node(JSON_INT);
        n->int_val = v;
    }
    free(tmp);
    return n;
}

static JsonNode *jp_parse_array(JsonParser *p) {
    p->pos++; // skip [
    JsonNode *n = json_new_node(JSON_ARRAY);
    n->array.cap = 4;
    n->array.items = (JsonNode **)calloc(4, sizeof(JsonNode *));
    jp_skip_ws(p);
    if (p->pos < p->len && p->src[p->pos] == ']') {
        p->pos++;
        return n;
    }
    while (1) {
        JsonNode *item = jp_parse_value(p);
        if (!item) { json_free_tree(n); return NULL; }
        if (n->array.len >= n->array.cap) {
            n->array.cap *= 2;
            n->array.items = (JsonNode **)realloc(n->array.items, n->array.cap * sizeof(JsonNode *));
        }
        n->array.items[n->array.len++] = item;
        jp_skip_ws(p);
        if (p->pos < p->len && p->src[p->pos] == ',') { p->pos++; continue; }
        if (p->pos < p->len && p->src[p->pos] == ']') { p->pos++; return n; }
        json_free_tree(n);
        return NULL;
    }
}

static JsonNode *jp_parse_object(JsonParser *p) {
    p->pos++; // skip {
    JsonNode *n = json_new_node(JSON_OBJECT);
    n->object.cap = 4;
    n->object.keys = (char **)calloc(4, sizeof(char *));
    n->object.vals = (JsonNode **)calloc(4, sizeof(JsonNode *));
    jp_skip_ws(p);
    if (p->pos < p->len && p->src[p->pos] == '}') {
        p->pos++;
        return n;
    }
    while (1) {
        jp_skip_ws(p);
        JsonNode *key_node = jp_parse_string_node(p);
        if (!key_node) { json_free_tree(n); return NULL; }
        jp_skip_ws(p);
        if (p->pos >= p->len || p->src[p->pos] != ':') { json_free_tree(key_node); json_free_tree(n); return NULL; }
        p->pos++; // skip :
        JsonNode *val = jp_parse_value(p);
        if (!val) { json_free_tree(key_node); json_free_tree(n); return NULL; }
        if (n->object.len >= n->object.cap) {
            n->object.cap *= 2;
            n->object.keys = (char **)realloc(n->object.keys, n->object.cap * sizeof(char *));
            n->object.vals = (JsonNode **)realloc(n->object.vals, n->object.cap * sizeof(JsonNode *));
        }
        n->object.keys[n->object.len] = key_node->string_val;
        key_node->string_val = NULL; // transfer ownership
        n->object.vals[n->object.len] = val;
        n->object.len++;
        free(key_node); // free node shell only (string_val transferred)
        jp_skip_ws(p);
        if (p->pos < p->len && p->src[p->pos] == ',') { p->pos++; continue; }
        if (p->pos < p->len && p->src[p->pos] == '}') { p->pos++; return n; }
        json_free_tree(n);
        return NULL;
    }
}

static JsonNode *jp_parse_value(JsonParser *p) {
    jp_skip_ws(p);
    if (p->pos >= p->len) return NULL;
    char c = p->src[p->pos];
    if (c == '"') return jp_parse_string_node(p);
    if (c == '{') return jp_parse_object(p);
    if (c == '[') return jp_parse_array(p);
    if (c == 't') {
        if (jp_match(p, "true")) { JsonNode *n = json_new_node(JSON_BOOL); n->bool_val = 1; return n; }
        return NULL;
    }
    if (c == 'f') {
        if (jp_match(p, "false")) { JsonNode *n = json_new_node(JSON_BOOL); n->bool_val = 0; return n; }
        return NULL;
    }
    if (c == 'n') {
        if (jp_match(p, "null")) return json_new_node(JSON_NULL);
        return NULL;
    }
    if (c == '-' || (c >= '0' && c <= '9')) return jp_parse_number(p);
    return NULL;
}

// Public parse: returns GC handle or sets error
void *__pluto_json_parse(void *pluto_str) {
    long slen = *(long *)pluto_str;
    const char *data = (const char *)pluto_str + 8;
    JsonParser parser = { data, 0, (int)slen };
    JsonNode *root = jp_parse_value(&parser);
    if (!root) {
        const char *msg = "invalid JSON";
        void *msg_str = __pluto_string_new(msg, (long)strlen(msg));
        void *err_obj = __pluto_alloc(8);
        *(long *)err_obj = (long)msg_str;
        __pluto_raise_error(err_obj);
        return json_wrap(json_new_node(JSON_NULL));
    }
    // Check for trailing non-whitespace
    jp_skip_ws(&parser);
    if (parser.pos < parser.len) {
        json_free_tree(root);
        const char *msg = "unexpected trailing content";
        void *msg_str = __pluto_string_new(msg, (long)strlen(msg));
        void *err_obj = __pluto_alloc(8);
        *(long *)err_obj = (long)msg_str;
        __pluto_raise_error(err_obj);
        return json_wrap(json_new_node(JSON_NULL));
    }
    return json_wrap(root);
}

// ── JSON stringify ────────────────────────────────────────────────────────────

typedef struct {
    char *buf;
    int len;
    int cap;
} StrBuf;

static void sb_init(StrBuf *sb) {
    sb->cap = 128;
    sb->buf = (char *)malloc(sb->cap);
    sb->len = 0;
}

static void sb_push(StrBuf *sb, const char *s, int slen) {
    while (sb->len + slen >= sb->cap) {
        sb->cap *= 2;
        sb->buf = (char *)realloc(sb->buf, sb->cap);
    }
    memcpy(sb->buf + sb->len, s, slen);
    sb->len += slen;
}

static void sb_push_char(StrBuf *sb, char c) {
    sb_push(sb, &c, 1);
}

static void sb_push_str(StrBuf *sb, const char *s) {
    sb_push(sb, s, (int)strlen(s));
}

static void json_stringify_node(JsonNode *n, StrBuf *sb) {
    if (!n) { sb_push_str(sb, "null"); return; }
    switch (n->type) {
    case JSON_NULL:
        sb_push_str(sb, "null");
        break;
    case JSON_BOOL:
        sb_push_str(sb, n->bool_val ? "true" : "false");
        break;
    case JSON_INT: {
        char tmp[32];
        int l = snprintf(tmp, sizeof(tmp), "%ld", (long)n->int_val);
        sb_push(sb, tmp, l);
        break;
    }
    case JSON_FLOAT: {
        char tmp[64];
        int l = snprintf(tmp, sizeof(tmp), "%.17g", n->float_val);
        sb_push(sb, tmp, l);
        break;
    }
    case JSON_STRING: {
        sb_push_char(sb, '"');
        const char *s = n->string_val;
        while (*s) {
            switch (*s) {
            case '"':  sb_push_str(sb, "\\\""); break;
            case '\\': sb_push_str(sb, "\\\\"); break;
            case '\b': sb_push_str(sb, "\\b"); break;
            case '\f': sb_push_str(sb, "\\f"); break;
            case '\n': sb_push_str(sb, "\\n"); break;
            case '\r': sb_push_str(sb, "\\r"); break;
            case '\t': sb_push_str(sb, "\\t"); break;
            default:
                if ((unsigned char)*s < 0x20) {
                    char esc[8];
                    snprintf(esc, sizeof(esc), "\\u%04x", (unsigned char)*s);
                    sb_push_str(sb, esc);
                } else {
                    sb_push_char(sb, *s);
                }
            }
            s++;
        }
        sb_push_char(sb, '"');
        break;
    }
    case JSON_ARRAY:
        sb_push_char(sb, '[');
        for (int i = 0; i < n->array.len; i++) {
            if (i > 0) sb_push_char(sb, ',');
            json_stringify_node(n->array.items[i], sb);
        }
        sb_push_char(sb, ']');
        break;
    case JSON_OBJECT:
        sb_push_char(sb, '{');
        for (int i = 0; i < n->object.len; i++) {
            if (i > 0) sb_push_char(sb, ',');
            // Key
            sb_push_char(sb, '"');
            const char *k = n->object.keys[i];
            while (*k) {
                if (*k == '"') sb_push_str(sb, "\\\"");
                else if (*k == '\\') sb_push_str(sb, "\\\\");
                else sb_push_char(sb, *k);
                k++;
            }
            sb_push_char(sb, '"');
            sb_push_char(sb, ':');
            json_stringify_node(n->object.vals[i], sb);
        }
        sb_push_char(sb, '}');
        break;
    }
}

void *__pluto_json_stringify(void *handle) {
    JsonNode *n = json_unwrap(handle);
    StrBuf sb;
    sb_init(&sb);
    json_stringify_node(n, &sb);
    void *result = __pluto_string_new(sb.buf, sb.len);
    free(sb.buf);
    return result;
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

typedef struct {
    pthread_mutex_t mutex;
    pthread_cond_t cond;
} TaskSync;

// Task handle layout (40 bytes, 5 slots):
//   [0] closure  (i64, GC pointer)
//   [1] result   (i64)
//   [2] error    (i64, GC pointer)
//   [3] done     (i64)
//   [4] sync_ptr (i64, raw malloc)

static void *__pluto_spawn_trampoline(void *arg) {
    long *task = (long *)arg;
    long closure_ptr = task[0];
    __pluto_current_error = NULL;  // clean TLS for new thread

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
    pthread_cond_signal(&sync->cond);
    pthread_mutex_unlock(&sync->mutex);

    atomic_fetch_sub(&__pluto_active_tasks, 1);
    return NULL;
}

long __pluto_task_spawn(long closure_ptr) {
    long *task = (long *)gc_alloc(40, GC_TAG_TASK, 3);
    task[0] = closure_ptr;
    task[1] = 0;  task[2] = 0;  task[3] = 0;

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

    if (task[2]) {
        __pluto_current_error = (void *)task[2];
        return 0;
    }
    return task[1];
}
