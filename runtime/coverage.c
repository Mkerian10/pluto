// ═══════════════════════════════════════════════════════════════════════════
// Pluto Runtime — Code Coverage
// ═══════════════════════════════════════════════════════════════════════════
//
// Provides counter-based code coverage instrumentation:
//   • __pluto_coverage_init(num_points, path_ptr) — allocate counter array, register atexit
//   • __pluto_coverage_hit(point_id) — increment counter for a coverage point
//   • __pluto_coverage_dump() — write binary counter data to disk (called via atexit)
//
// Binary format (.pluto-coverage/coverage-data.bin):
//   [num_points: i64][counter_0: i64][counter_1: i64]...[counter_N-1: i64]
//
// ═══════════════════════════════════════════════════════════════════════════

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <sys/stat.h>
#include <libgen.h>

static int64_t *__coverage_counters = NULL;
static int64_t __coverage_num_points = 0;
static char *__coverage_output_path = NULL;
static int __coverage_initialized = 0;

// Forward declaration
static void __pluto_coverage_dump(void);

/// Initialize coverage tracking.
/// `num_points` — number of coverage points to track
/// `path_ptr` — Pluto string object (pointer to GC-allocated string)
void __pluto_coverage_init(int64_t num_points, int64_t path_ptr) {
    if (__coverage_initialized) return;
    __coverage_initialized = 1;

    __coverage_num_points = num_points;
    __coverage_counters = (int64_t *)calloc(num_points, sizeof(int64_t));
    if (!__coverage_counters) {
        fprintf(stderr, "coverage: failed to allocate %lld counters\n", (long long)num_points);
        return;
    }

    // Extract C string from Pluto string object.
    // Pluto strings are GC objects: [GCHeader][len:i64][data...]
    // The data pointer is at offset sizeof(GCHeader) + 8 bytes (after len field).
    // But for coverage we receive a raw C string pointer from codegen (not a Pluto string).
    // The codegen passes a raw pointer to a null-terminated C string.
    const char *raw = (const char *)path_ptr;
    size_t len = strlen(raw);
    __coverage_output_path = (char *)malloc(len + 1);
    if (__coverage_output_path) {
        memcpy(__coverage_output_path, raw, len + 1);
    }

    atexit(__pluto_coverage_dump);
}

/// Increment the counter for a coverage point.
void __pluto_coverage_hit(int64_t point_id) {
    if (__coverage_counters && point_id >= 0 && point_id < __coverage_num_points) {
        __coverage_counters[point_id]++;
    }
}

/// Write binary counter data to disk. Called via atexit().
static void __pluto_coverage_dump(void) {
    if (!__coverage_counters || !__coverage_output_path) return;

    // Ensure parent directory exists
    char *path_copy = strdup(__coverage_output_path);
    if (path_copy) {
        char *dir = dirname(path_copy);
        mkdir(dir, 0755);
        free(path_copy);
    }

    FILE *f = fopen(__coverage_output_path, "wb");
    if (!f) {
        fprintf(stderr, "coverage: failed to open '%s' for writing\n", __coverage_output_path);
        goto cleanup;
    }

    // Write: [num_points][counter_0][counter_1]...[counter_N-1]
    fwrite(&__coverage_num_points, sizeof(int64_t), 1, f);
    fwrite(__coverage_counters, sizeof(int64_t), __coverage_num_points, f);
    fclose(f);

cleanup:
    free(__coverage_counters);
    __coverage_counters = NULL;
    free(__coverage_output_path);
    __coverage_output_path = NULL;
}
