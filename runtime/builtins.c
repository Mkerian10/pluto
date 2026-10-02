//──────────────────────────────────────────────────────────────────────────────
// Pluto Runtime: Core Builtins
//
// Core runtime utilities (strings, arrays, I/O, collections).
//
// Contents:
// - Print functions (stdout output)
// - String operations (allocation, concatenation, slicing, parsing)
// - Array operations (dynamic arrays with push/get/set/length)
// - Bytes operations (byte array manipulation)
// - Map/Set operations (hash tables with open addressing)
// - File I/O (read, write, exists, delete)
// - Socket I/O (TCP client/server, UDP)
// - HTTP client (simple GET/POST)
// - Math builtins (trigonometry, rounding)
// - Test framework (expect assertions)
// - Error handling (TLS error state)
// - Contract enforcement (__pluto_requires_violation, __pluto_assert_failure)
// - RPC response parsing (JSON extraction)
//──────────────────────────────────────────────────────────────────────────────

#include "builtins.h"

// ── Print functions ───────────────────────────────────────────────────────────

// Line-buffer stdout (once) so output is flushed on each newline even when
// stdout is a pipe. Without this, a process whose stdout is captured by a
// parent (e.g. a server printing its port before blocking on accept) would
// have its output stuck in a full buffer until exit, deadlocking the reader.
static void __pluto_ensure_line_buffered(void) {
    static int done = 0;
    if (!done) {
        setvbuf(stdout, NULL, _IOLBF, 0);
        done = 1;
    }
}

void __pluto_print_int(long value) {
    __pluto_ensure_line_buffered();
    printf("%ld\n", value);
}

// Deterministic float formatting: canonical "inf"/"-inf"/"nan" for special
// values; otherwise the shortest decimal digit string that parses back to the
// exact same double (minimal significant digits found by widening %e until
// the value round-trips), rendered in fixed notation for decimal exponents
// -4..15 and scientific notation outside that range. Both libcs we target
// (glibc, Apple libc) produce identical correctly-rounded %e/%f output in the
// C locale, and the runtime never calls setlocale, so the result is
// platform-independent. Returns the length.
static int __pluto_format_double(double value, char *buf, size_t cap) {
    if (isnan(value)) {
        return snprintf(buf, cap, "nan");
    }
    if (isinf(value)) {
        return snprintf(buf, cap, value < 0 ? "-inf" : "inf");
    }
    char sci[40];
    int digits = 17;
    for (int d = 1; d <= 17; d++) {
        snprintf(sci, sizeof sci, "%.*e", d - 1, value);
        if (strtod(sci, NULL) == value) {
            digits = d;
            break;
        }
    }
    snprintf(sci, sizeof sci, "%.*e", digits - 1, value);
    int exp = atoi(strchr(sci, 'e') + 1);
    if (exp >= -4 && exp < 16) {
        // Fixed notation with exactly the significant digits found above.
        // Integer parts here are < 10^16 < 2^53, so %f prints them exactly.
        int prec = digits - 1 - exp;
        if (prec < 0) {
            prec = 0;
        }
        return snprintf(buf, cap, "%.*f", prec, value);
    }
    return snprintf(buf, cap, "%s", sci);
}

void __pluto_print_float(double value) {
    __pluto_ensure_line_buffered();
    char buf[40];
    __pluto_format_double(value, buf, sizeof buf);
    printf("%s\n", buf);
}

void __pluto_print_string(void *header) {
    __pluto_ensure_line_buffered();
    const char *data;
    long len;
    __pluto_string_data(header, &data, &len);
    printf("%.*s\n", (int)len, data);
}

void __pluto_print_bool(int value) {
    __pluto_ensure_line_buffered();
    printf("%s\n", value ? "true" : "false");
}

void __pluto_print_string_no_newline(void *header) {
    __pluto_ensure_line_buffered();
    const char *data;
    long len;
    __pluto_string_data(header, &data, &len);
    printf("%.*s", (int)len, data);
}

// ── Memory allocation ─────────────────────────────────────────────────────────

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
    const char *data_a, *data_b;
    long len_a, len_b;
    __pluto_string_data(a, &data_a, &len_a);
    __pluto_string_data(b, &data_b, &len_b);
    if (len_a > LONG_MAX - len_b) {
        fprintf(stderr, "pluto: string concatenation overflow\n");
        exit(1);
    }
    long total = len_a + len_b;
    size_t alloc_size = 8 + total + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = total;
    memcpy((char *)header + 8, data_a, len_a);
    memcpy((char *)header + 8 + len_a, data_b, len_b);
    ((char *)header)[8 + total] = '\0';
    return header;
}

int __pluto_string_eq(void *a, void *b) {
    const char *data_a, *data_b;
    long len_a, len_b;
    __pluto_string_data(a, &data_a, &len_a);
    __pluto_string_data(b, &data_b, &len_b);
    if (len_a != len_b) return 0;
    return memcmp(data_a, data_b, len_a) == 0 ? 1 : 0;
}

long __pluto_string_len(void *s) {
    const char *data;
    long len;
    __pluto_string_data(s, &data, &len);
    return len;
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
    const char *str_data;
    long len;
    __pluto_string_data(s, &str_data, &len);
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
    const char *data;
    long slen;
    __pluto_string_data(s, &data, &slen);
    if (start < 0) start = 0;
    if (start > slen) start = slen;
    if (len < 0) len = 0;
    if (start + len > slen) len = slen - start;
    return __pluto_string_slice_new(s, start, len);
}

long __pluto_string_contains(void *haystack, void *needle) {
    const char *hdata, *ndata;
    long hlen, nlen;
    __pluto_string_data(haystack, &hdata, &hlen);
    __pluto_string_data(needle, &ndata, &nlen);
    if (nlen == 0) return 1;
    if (nlen > hlen) return 0;
    return memmem(hdata, hlen, ndata, nlen) != NULL ? 1 : 0;
}

long __pluto_string_starts_with(void *s, void *prefix) {
    const char *sdata, *pdata;
    long slen, plen;
    __pluto_string_data(s, &sdata, &slen);
    __pluto_string_data(prefix, &pdata, &plen);
    if (plen == 0) return 1;
    if (plen > slen) return 0;
    return memcmp(sdata, pdata, plen) == 0 ? 1 : 0;
}

long __pluto_string_ends_with(void *s, void *suffix) {
    const char *sdata, *sfxdata;
    long slen, sfxlen;
    __pluto_string_data(s, &sdata, &slen);
    __pluto_string_data(suffix, &sfxdata, &sfxlen);
    if (sfxlen == 0) return 1;
    if (sfxlen > slen) return 0;
    return memcmp(sdata + slen - sfxlen, sfxdata, sfxlen) == 0 ? 1 : 0;
}

long __pluto_string_index_of(void *haystack, void *needle) {
    const char *hdata, *ndata;
    long hlen, nlen;
    __pluto_string_data(haystack, &hdata, &hlen);
    __pluto_string_data(needle, &ndata, &nlen);
    if (nlen == 0) return 0;
    if (nlen > hlen) return -1;
    const char *found = (const char *)memmem(hdata, hlen, ndata, nlen);
    if (!found) return -1;
    return (long)(found - hdata);
}

void *__pluto_string_trim(void *s) {
    const char *data;
    long slen;
    __pluto_string_data(s, &data, &slen);
    long start = 0;
    long end = slen;
    while (start < end && (data[start] == ' ' || data[start] == '\t' || data[start] == '\n' || data[start] == '\r')) start++;
    while (end > start && (data[end-1] == ' ' || data[end-1] == '\t' || data[end-1] == '\n' || data[end-1] == '\r')) end--;
    long newlen = end - start;
    return __pluto_string_slice_new(s, start, newlen);
}

void *__pluto_string_to_upper(void *s) {
    const char *data;
    long slen;
    __pluto_string_data(s, &data, &slen);
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
    const char *data;
    long slen;
    __pluto_string_data(s, &data, &slen);
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
    const char *sdata, *odata, *ndata;
    long slen, olen, nlen;
    __pluto_string_data(s, &sdata, &slen);
    __pluto_string_data(old, &odata, &olen);
    __pluto_string_data(new_str, &ndata, &nlen);
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
    const char *sdata, *ddata;
    long slen, dlen;
    __pluto_string_data(s, &sdata, &slen);
    __pluto_string_data(delim, &ddata, &dlen);
    void *arr = __pluto_array_new(4);
    if (dlen == 0) {
        for (long i = 0; i < slen; i++) {
            void *ch = __pluto_string_slice_new(s, i, 1);
            __pluto_array_push(arr, (long)ch);
        }
        return arr;
    }
    long pos = 0;
    long remaining = slen;
    while (1) {
        if (remaining < dlen) {
            __pluto_array_push(arr, (long)__pluto_string_slice_new(s, pos, remaining));
            break;
        }
        const char *found = (const char *)memmem(sdata + pos, remaining, ddata, dlen);
        if (!found) {
            __pluto_array_push(arr, (long)__pluto_string_slice_new(s, pos, remaining));
            break;
        }
        long seglen = found - (sdata + pos);
        __pluto_array_push(arr, (long)__pluto_string_slice_new(s, pos, seglen));
        pos += seglen + dlen;
        remaining -= seglen + dlen;
    }
    return arr;
}

void *__pluto_string_char_at(void *s, long index) {
    const char *data;
    long slen;
    __pluto_string_data(s, &data, &slen);
    if (index < 0 || index >= slen) {
        fprintf(stderr, "pluto: string index out of bounds: index %ld, length %ld\n", index, slen);
        exit(1);
    }
    void *header = gc_alloc(8 + 1 + 1, GC_TAG_STRING, 0);
    *(long *)header = 1;
    ((char *)header)[8] = data[index];
    ((char *)header)[9] = '\0';
    return header;
}

long __pluto_string_byte_at(void *s, long index) {
    const char *data;
    long slen;
    __pluto_string_data(s, &data, &slen);
    if (index < 0 || index >= slen) {
        fprintf(stderr, "pluto: string byte_at index out of bounds: index %ld, length %ld\n", index, slen);
        exit(1);
    }
    return (long)(unsigned char)data[index];
}

void *__pluto_string_format_float(double value) {
    char buf[40];
    int len = __pluto_format_double(value, buf, sizeof buf);
    size_t alloc_size = 8 + len + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = len;
    memcpy((char *)header + 8, buf, len + 1);
    return header;
}

void *__pluto_string_to_int(void *s) {
    const char *data;
    long slen;
    __pluto_string_data(s, &data, &slen);
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
    const char *data;
    long slen;
    __pluto_string_data(s, &data, &slen);
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
    const char *data;
    long slen;
    __pluto_string_data(s, &data, &slen);
    long start_idx = 0;
    while (start_idx < slen && (data[start_idx] == ' ' || data[start_idx] == '\t' || data[start_idx] == '\n' || data[start_idx] == '\r')) {
        start_idx++;
    }
    long new_len = slen - start_idx;
    return __pluto_string_slice_new(s, start_idx, new_len);
}

void *__pluto_string_trim_end(void *s) {
    const char *data;
    long slen;
    __pluto_string_data(s, &data, &slen);
    long end_idx = slen - 1;
    while (end_idx >= 0 && (data[end_idx] == ' ' || data[end_idx] == '\t' || data[end_idx] == '\n' || data[end_idx] == '\r')) {
        end_idx--;
    }
    long new_len = end_idx + 1;
    if (new_len < 0) new_len = 0;
    return __pluto_string_slice_new(s, 0, new_len);
}

long __pluto_string_last_index_of(void *haystack, void *needle) {
    const char *hdata, *ndata;
    long hlen, nlen;
    __pluto_string_data(haystack, &hdata, &hlen);
    __pluto_string_data(needle, &ndata, &nlen);
    if (nlen == 0) return hlen;
    if (nlen > hlen) return -1;

    for (long i = hlen - nlen; i >= 0; i--) {
        if (memcmp(hdata + i, ndata, nlen) == 0) {
            return i;
        }
    }
    return -1;
}

long __pluto_string_count(void *haystack, void *needle) {
    const char *hdata, *ndata;
    long hlen, nlen;
    __pluto_string_data(haystack, &hdata, &hlen);
    __pluto_string_data(needle, &ndata, &nlen);
    if (nlen == 0) return 0;
    if (nlen > hlen) return 0;

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
    const char *data;
    long slen;
    __pluto_string_data(s, &data, &slen);
    return slen == 0 ? 1 : 0;
}

long __pluto_string_is_whitespace(void *s) {
    const char *data;
    long slen;
    __pluto_string_data(s, &data, &slen);
    if (slen == 0) return 1;
    for (long i = 0; i < slen; i++) {
        if (data[i] != ' ' && data[i] != '\t' && data[i] != '\n' && data[i] != '\r') {
            return 0;
        }
    }
    return 1;
}

void *__pluto_string_repeat(void *s, long count) {
    const char *data;
    long slen;
    __pluto_string_data(s, &data, &slen);
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
    const char *cstr = __pluto_string_to_cstr(s);
    return strtol(cstr, NULL, 10);
}

double __pluto_json_parse_float(void *s) {
    const char *cstr = __pluto_string_to_cstr(s);
    return strtod(cstr, NULL);
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
    char buf[40];
    int len = __pluto_format_double(value, buf, sizeof buf);
    size_t alloc_size = 8 + len + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = len;
    memcpy((char *)header + 8, buf, len + 1);
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

// ── String slice functions ────────────────────────────────────────────────────
// String slices are lightweight 24-byte views into owned strings: [backing_ptr][offset][len]
// They avoid copying on substring/trim/split operations. Slices are materialized
// (copied to owned) when escaping scope (stored in structs, arrays, closures, returned).

// Extract (data_ptr, len) from either an owned string or a string slice.
void __pluto_string_data(void *s, const char **data_out, long *len_out) {
    GCHeader *h = (GCHeader *)((char *)s - sizeof(GCHeader));
    if (h->type_tag == GC_TAG_STRING_SLICE) {
        long *slice = (long *)s;
        void *backing = (void *)slice[0];
        long offset = slice[1];
        long len = slice[2];
        *data_out = (const char *)backing + 8 + offset;
        *len_out = len;
    } else {
        *data_out = (const char *)s + 8;
        *len_out = *(long *)s;
    }
}

// Create a new string slice. Returns empty owned string for len==0.
// Flattens slice-of-slice: if backing is itself a slice, points to original backing.
void *__pluto_string_slice_new(void *backing, long offset, long len) {
    if (len <= 0) {
        return __pluto_string_new("", 0);
    }
    // Flatten slice-of-slice: always point to the original owned string
    void *real_backing = backing;
    long real_offset = offset;
    GCHeader *h = (GCHeader *)((char *)backing - sizeof(GCHeader));
    if (h->type_tag == GC_TAG_STRING_SLICE) {
        long *parent_slice = (long *)backing;
        real_backing = (void *)parent_slice[0];
        real_offset = parent_slice[1] + offset;
    }
    long *slice = (long *)gc_alloc(24, GC_TAG_STRING_SLICE, 1);
    slice[0] = (long)real_backing;
    slice[1] = real_offset;
    slice[2] = len;
    return slice;
}

// Materialize a slice to an owned string. No-op if already owned.
void *__pluto_string_slice_to_owned(void *s) {
    if (!s) return s;
    GCHeader *h = (GCHeader *)((char *)s - sizeof(GCHeader));
    if (h->type_tag != GC_TAG_STRING_SLICE) return s;
    long *slice = (long *)s;
    void *backing = (void *)slice[0];
    long offset = slice[1];
    long len = slice[2];
    const char *data = (const char *)backing + 8 + offset;
    return __pluto_string_new(data, len);
}

// Null-safe escape wrapper: materializes slices, passes through owned strings.
// Called by codegen at escape boundaries (return, struct field, array element, etc.)
void *__pluto_string_escape(void *s) {
    if (!s) return s;
    return __pluto_string_slice_to_owned(s);
}

// Returns a null-terminated C string pointer. For owned strings, returns data directly.
// For slices, materializes to owned first (since slices lack null terminators).
const char *__pluto_string_to_cstr(void *s) {
    if (!s) return "";
    GCHeader *h = (GCHeader *)((char *)s - sizeof(GCHeader));
    if (h->type_tag == GC_TAG_STRING_SLICE) {
        void *owned = __pluto_string_slice_to_owned(s);
        return (const char *)owned + 8;
    }
    return (const char *)s + 8;
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
    __pluto_current_error_type = NULL;
}

// Record the type name of the currently-raised error (a pluto string), so a
// typed `catch ... : T` can discriminate which error is in flight.
void __pluto_set_error_type(void *type_str) {
    __pluto_current_error_type = type_str;
}

// The type name of the current error, or "" if none/untyped.
void *__pluto_error_type() {
    return __pluto_current_error_type ? __pluto_current_error_type : __pluto_string_new("", 0);
}

// ── Unhandled-error exit check ────────────────────────────────────────────────
// An error that reaches the end of main with no handler must not vanish
// silently (a select with all channels closed, a leaked error through an
// untyped escape, ...). Registered via atexit from __pluto_gc_init: if the
// main thread's error slot is still occupied when the process exits normally,
// report it and fail the process.
static void __pluto_unhandled_error_exit_check(void) {
    if (__pluto_current_error) {
        fflush(NULL);
        if (__pluto_current_error_type) {
            long *type_ps = (long *)__pluto_current_error_type;
            long len = type_ps[0];
            const char *data = (const char *)&type_ps[1];
            fprintf(stderr, "pluto: unhandled error escaped main: %.*s\n", (int)len, data);
        } else {
            fprintf(stderr, "pluto: unhandled error escaped main\n");
        }
        _exit(1);
    }
}

void __pluto_register_exit_check(void) {
    atexit(__pluto_unhandled_error_exit_check);
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
#ifdef PLUTO_TEST_MODE
    // Test mode: sleep is the degenerate timed wait — a yield point that
    // resumes when the scheduler chooses (duration erased). A real nanosleep
    // here would stall the entire single-threaded fiber scheduler without
    // ever yielding, providing no ordering and burning wall clock.
    (void)ns;
    __pluto_test_timed_yield();
#else
    struct timespec req;
    req.tv_sec = ns / 1000000000L;
    req.tv_nsec = ns % 1000000000L;
    __pluto_gc_enter_safe_region();
    nanosleep(&req, NULL);
    __pluto_gc_leave_safe_region();
#endif
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
    return (long)__pluto_gc_bytes_allocated();
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
    const char *host = __pluto_string_to_cstr(host_str);
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
    __pluto_gc_enter_safe_region();
    long conn = (long)accept((int)fd, (struct sockaddr *)&client_addr, &client_len);
    __pluto_gc_leave_safe_region();
    return conn;
}

long __pluto_socket_connect(long fd, void *host_str, long port) {
    const char *host = __pluto_string_to_cstr(host_str);
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_port = htons((uint16_t)port);
    if (inet_pton(AF_INET, host, &addr.sin_addr) != 1) return -1;
    __pluto_gc_enter_safe_region();
    int rc = connect((int)fd, (struct sockaddr *)&addr, sizeof(addr));
    __pluto_gc_leave_safe_region();
    return rc == 0 ? 0 : -1;
}

// Read-deadline support (issue #370). SO_RCVTIMEO is the mechanism
// __pluto_serve_accept already uses for its hardcoded 5s guard; this exposes
// it as a per-connection setting. A timed-out read is reported through a
// thread-local flag so the stdlib can raise a typed TimedOut distinct from
// connection errors — a plain socket-read timeout is a LOCAL fact ("no bytes
// arrived within the deadline"), never a claim about the peer.
static __thread int socket_read_timed_out = 0;

long __pluto_socket_set_read_timeout(long fd, long ms) {
    struct timeval tv;
    if (ms < 0) ms = 0;  // 0 clears the deadline (kernel semantics)
    tv.tv_sec = ms / 1000;
    tv.tv_usec = (ms % 1000) * 1000;
    return setsockopt((int)fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv)) == 0 ? 0 : -1;
}

long __pluto_socket_read_timed_out(void) {
    return socket_read_timed_out;
}

void *__pluto_socket_read(long fd, long max_bytes) {
    socket_read_timed_out = 0;
    if (max_bytes <= 0) {
        return __pluto_string_new("", 0);
    }
    if (max_bytes > 1048576) max_bytes = 1048576;
    char *buf = (char *)malloc(max_bytes);
    if (!buf) return __pluto_string_new("", 0);
    __pluto_gc_enter_safe_region();
    ssize_t n = read((int)fd, buf, (size_t)max_bytes);
    int read_errno = errno;
    __pluto_gc_leave_safe_region();
    if (n <= 0) {
        if (n < 0 && (read_errno == EAGAIN || read_errno == EWOULDBLOCK)) {
            socket_read_timed_out = 1;
        }
        free(buf);
        return __pluto_string_new("", 0);
    }
    void *result = __pluto_string_new(buf, n);
    free(buf);
    return result;
}

long __pluto_socket_write(long fd, void *data_str) {
    const char *data;
    long len;
    __pluto_string_data(data_str, &data, &len);
    return (long)write((int)fd, data, (size_t)len);
}

// Bytes-typed socket I/O: identical syscall path to the string variants, only
// the handle type at the boundary changes. Read lands in a scratch buffer
// FIRST and the GC handle is allocated after — the read blocks in a GC safe
// region, so no GC-visible allocation may be in flight across it.
long __pluto_socket_read_bytes(long fd, long max_bytes) {
    char *buf = NULL;
    ssize_t n = 0;
    if (max_bytes > 0) {
        if (max_bytes > 1048576) max_bytes = 1048576;
        buf = (char *)malloc((size_t)max_bytes);
        if (buf) {
            __pluto_gc_enter_safe_region();
            n = read((int)fd, buf, (size_t)max_bytes);
            __pluto_gc_leave_safe_region();
        }
    }
    if (n < 0) n = 0; // EOF and error both yield empty bytes (parity with read)
    long *handle = (long *)gc_alloc(24, GC_TAG_BYTES, 3);
    long cap = n > 16 ? n : 16;
    unsigned char *data = (unsigned char *)malloc((size_t)cap);
    if (!data) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    if (n > 0) memcpy(data, buf, (size_t)n);
    handle[0] = n;
    handle[1] = cap;
    handle[2] = (long)data;
    if (buf) free(buf);
    return (long)handle;
}

long __pluto_socket_write_bytes(long fd, long bytes_handle) {
    long *h = (long *)bytes_handle;
    long len = h[0];
    unsigned char *data = (unsigned char *)h[2];
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

// ── Remote calls (Phase 2 transport) ──────────────────────────────────────────
// Resolves the target service address from env PLUTO_REMOTE_<SERVICE> (uppercased),
// connects, sends "<method>\n<payload>", and returns the response string.
// Returns NULL on any failure so the caller can raise NetworkError.
// ── Length-framed messages ────────────────────────────────────────────────────
// A message is a 4-byte big-endian length followed by exactly that many bytes.
// This lets a reader recover the full message regardless of how TCP splits it
// across segments — necessary now that complex types produce multi-KB payloads.

// Write a length-framed message. Returns 0 on success, -1 on failure.
long __pluto_write_framed(long fd, void *str) {
    const char *data;
    long len;
    __pluto_string_data(str, &data, &len);
    unsigned char hdr[4];
    hdr[0] = (unsigned char)((len >> 24) & 0xff);
    hdr[1] = (unsigned char)((len >> 16) & 0xff);
    hdr[2] = (unsigned char)((len >> 8) & 0xff);
    hdr[3] = (unsigned char)(len & 0xff);
    long off = 0;
    while (off < 4) {
        __pluto_gc_enter_safe_region();
        ssize_t n = write((int)fd, hdr + off, (size_t)(4 - off));
        __pluto_gc_leave_safe_region();
        if (n <= 0) return -1;
        off += n;
    }
    off = 0;
    while (off < len) {
        __pluto_gc_enter_safe_region();
        ssize_t n = write((int)fd, data + off, (size_t)(len - off));
        __pluto_gc_leave_safe_region();
        if (n <= 0) return -1;
        off += n;
    }
    return 0;
}

// Whether the last __pluto_read_framed failure on this thread was a read
// deadline expiring (SO_RCVTIMEO) rather than EOF/reset. Lets the RPC client
// name the timeout in its (still ambiguous) classification message.
static __thread int framed_read_timed_out = 0;

// Read a length-framed message into a pluto string. Returns NULL on failure.
void *__pluto_read_framed(long fd) {
    framed_read_timed_out = 0;
    unsigned char hdr[4];
    long got = 0;
    while (got < 4) {
        __pluto_gc_enter_safe_region();
        ssize_t n = read((int)fd, hdr + got, (size_t)(4 - got));
        int read_errno = errno;
        __pluto_gc_leave_safe_region();
        if (n <= 0) {
            if (n < 0 && (read_errno == EAGAIN || read_errno == EWOULDBLOCK))
                framed_read_timed_out = 1;
            return NULL;
        }
        got += n;
    }
    long len = ((long)hdr[0] << 24) | ((long)hdr[1] << 16) | ((long)hdr[2] << 8) | (long)hdr[3];
    if (len < 0 || len > (64L * 1024 * 1024)) return NULL; // 64MB sanity cap
    char *buf = (char *)malloc(len > 0 ? (size_t)len : 1);
    if (!buf) return NULL;
    long off = 0;
    while (off < len) {
        __pluto_gc_enter_safe_region();
        ssize_t n = read((int)fd, buf + off, (size_t)(len - off));
        int read_errno = errno;
        __pluto_gc_leave_safe_region();
        if (n <= 0) {
            if (n < 0 && (read_errno == EAGAIN || read_errno == EWOULDBLOCK))
                framed_read_timed_out = 1;
            free(buf);
            return NULL;
        }
        off += n;
    }
    void *result = __pluto_string_new(buf, len);
    free(buf);
    return result;
}

// ── Entity registry & identity handles (rfc-objects.md phase 2) ─────────────
//
// Entities cross boundaries as handles (home|type|id). The registry maps ids
// to live pointers; exported entities are pinned as GC roots (release
// protocol is future work). Decoding a handle whose home is THIS process
// returns the live entity — identity survives the round trip. Foreign
// handles materialize as GC_TAG_HANDLE stubs; calling methods on a stub is
// a runtime error until handle-call routing ships.

static void **entity_registry = NULL;
static long entity_count = 0;
static long entity_cap = 0;
static char entity_home[64] = {0};

// Test mode runs a single-threaded fiber scheduler with no pthreads — the
// registry needs no lock there.
#ifdef PLUTO_TEST_MODE
#define ENTITY_LOCK()
#define ENTITY_UNLOCK()
#else
static pthread_mutex_t entity_mutex = PTHREAD_MUTEX_INITIALIZER;
#define ENTITY_LOCK() pthread_mutex_lock(&entity_mutex)
#define ENTITY_UNLOCK() pthread_mutex_unlock(&entity_mutex)
#endif

static const char *entity_home_str(void) {
    if (!entity_home[0]) {
        snprintf(entity_home, sizeof(entity_home), "H%ld-%ld",
                 (long)getpid(), (long)time(NULL));
    }
    return entity_home;
}

long __pluto_entity_export(void *ptr) {
    ENTITY_LOCK();
    for (long i = 0; i < entity_count; i++) {
        if (entity_registry[i] == ptr) {
            ENTITY_UNLOCK();
            return i + 1;
        }
    }
    if (entity_count == entity_cap) {
        entity_cap = entity_cap ? entity_cap * 2 : 16;
        entity_registry = (void **)realloc(entity_registry, (size_t)entity_cap * sizeof(void *));
    }
    entity_registry[entity_count] = ptr;
    __pluto_gc_add_pending_root(ptr);  // pinned: exported identity must outlive local refs
    long id = ++entity_count;
    ENTITY_UNLOCK();
    return id;
}

// "E<home>|<type>|<id>" — schema-level, newline-free
void *__pluto_entity_encode(void *ptr, void *type_str) {
    // Re-encoding a HANDLE forwards the original identity triple.
    GCHeader *h = (GCHeader *)((char *)ptr - sizeof(GCHeader));
    if (h->type_tag == GC_TAG_HANDLE) {
        long *slots = (long *)ptr;
        const char *home; long home_len;
        const char *ty; long ty_len;
        __pluto_string_data((void *)slots[0], &home, &home_len);
        __pluto_string_data((void *)slots[1], &ty, &ty_len);
        char buf[256];
        int n = snprintf(buf, sizeof(buf), "E%.*s|%.*s|%ld",
                         (int)home_len, home, (int)ty_len, ty, slots[2]);
        return __pluto_string_new(buf, n);
    }
    long id = __pluto_entity_export(ptr);
    const char *ty; long ty_len;
    __pluto_string_data(type_str, &ty, &ty_len);
    char buf[256];
    int n = snprintf(buf, sizeof(buf), "E%s|%.*s|%ld",
                     entity_home_str(), (int)ty_len, ty, id);
    return __pluto_string_new(buf, n);
}

void *__pluto_entity_decode(void *wire_str) {
    const char *sdata; long slen;
    __pluto_string_data(wire_str, &sdata, &slen);
    if (slen < 2 || sdata[0] != 'E') return NULL;
    // parse E<home>|<type>|<id>
    const char *p1 = memchr(sdata + 1, '|', (size_t)(slen - 1));
    if (!p1) return NULL;
    const char *p2 = memchr(p1 + 1, '|', (size_t)(sdata + slen - p1 - 1));
    if (!p2) return NULL;
    long home_len = p1 - (sdata + 1);
    long ty_len = p2 - (p1 + 1);
    long id = atol(p2 + 1);
    const char *home = entity_home_str();
    if ((long)strlen(home) == home_len && memcmp(home, sdata + 1, (size_t)home_len) == 0) {
        ENTITY_LOCK();
        void *live = (id >= 1 && id <= entity_count) ? entity_registry[id - 1] : NULL;
        ENTITY_UNLOCK();
        if (live) return live;
        return NULL;
    }
    // Foreign entity: materialize a handle stub
    void *home_s = __pluto_string_new((char *)(sdata + 1), home_len);
    void *ty_s = __pluto_string_new((char *)(p1 + 1), ty_len);
    long *stub = (long *)gc_alloc(24, GC_TAG_HANDLE, 3);
    stub[0] = (long)home_s;
    stub[1] = (long)ty_s;
    stub[2] = id;
    return stub;
}

// Guard before every object method call: a foreign-handle receiver cannot be
// invoked locally (routing is a later slice).
long __pluto_handle_is(void *ptr) {
    if (!ptr) return 0;
    GCHeader *h = (GCHeader *)((char *)ptr - sizeof(GCHeader));
    return h->type_tag == GC_TAG_HANDLE ? 1 : 0;
}

void __pluto_entity_guard(void *ptr) {
    if (!ptr) return;
    GCHeader *h = (GCHeader *)((char *)ptr - sizeof(GCHeader));
    if (h->type_tag == GC_TAG_HANDLE) {
        fprintf(stderr, "pluto: cannot call a method on a foreign entity handle: "
                        "calls must route to the entity's home domain "
                        "(not yet implemented — rfc-objects.md phase 2)\n");
        exit(1);
    }
}

// ── Boundary-failure classification (docs/design/epistemics.md) ──────────────
// Every failed boundary call is classified at the transport layer — the only
// layer that knows whether the request frame left the process:
//   definite = 1  the request is KNOWN not to have been dispatched: nothing was
//                 sent, or the length-framed request frame cannot have been
//                 completed (the server dispatches only after reading a full
//                 frame, so an incomplete frame is never executed).
//   definite = 0  the full request frame was handed to the kernel and no
//                 response came back. The effect may or may not have applied;
//                 no local information can say. (Ambiguous.)
// The default is 0: when in doubt, the runtime claims ignorance, never
// knowledge. Thread-local like the error registers: each handler thread/task
// classifies its own boundary calls.
static __thread long boundary_definite = 0;
static __thread const char *boundary_reason = "";

static void pluto_boundary_fail(long definite, const char *reason) {
    boundary_definite = definite;
    boundary_reason = reason;
}

long __pluto_boundary_failure_definite(void) { return boundary_definite; }

void *__pluto_boundary_failure_reason(void) {
    return __pluto_string_new(boundary_reason, (long)strlen(boundary_reason));
}

static void *pluto_request_to_addr(const char *addr, void *method_str, void *payload_str);

static void *pluto_boundary_request(const char *prefix, void *service_str, void *method_str, void *payload_str) {
    const char *svc;
    long svclen;
    __pluto_string_data(service_str, &svc, &svclen);

    char envname[256];
    int n = 0;
    while (prefix[n]) { envname[n] = prefix[n]; n++; }
    for (long i = 0; i < svclen && n < (int)sizeof(envname) - 1; i++) {
        char c = svc[i];
        if (c >= 'a' && c <= 'z') c -= 32;
        envname[n++] = c;
    }
    envname[n] = 0;

    const char *addr = getenv(envname);
    if (!addr) {
        pluto_boundary_fail(1, "service address not configured (nothing sent)");
        return NULL;
    }
    return pluto_request_to_addr(addr, method_str, payload_str);
}

/* Handle-call routing (rfc-objects.md phase 2 slice 2): dial an entity's
 * home address directly — the handle carries it. */
void *__pluto_entity_request(void *home_str, void *method_str, void *payload_str) {
    const char *home;
    long hlen;
    __pluto_string_data(home_str, &home, &hlen);
    char addr[160];
    if (hlen <= 0 || hlen >= (long)sizeof(addr)) {
        pluto_boundary_fail(1, "invalid entity home address (nothing sent)");
        return NULL;
    }
    memcpy(addr, home, (size_t)hlen);
    addr[hlen] = 0;
    return pluto_request_to_addr(addr, method_str, payload_str);
}

static void *pluto_request_to_addr(const char *addr, void *method_str, void *payload_str) {
    const char *colon = strchr(addr, ':');
    if (!colon) {
        pluto_boundary_fail(1, "malformed service address (nothing sent)");
        return NULL;
    }
    size_t hlen = (size_t)(colon - addr);
    char host[128];
    if (hlen == 0 || hlen >= sizeof(host)) {
        pluto_boundary_fail(1, "malformed service address (nothing sent)");
        return NULL;
    }
    memcpy(host, addr, hlen);
    host[hlen] = 0;
    long port = atol(colon + 1);

    long fd = __pluto_socket_create(2, 1, 0);
    if (fd < 0) {
        pluto_boundary_fail(1, "socket creation failed (nothing sent)");
        return NULL;
    }
    void *host_ps = __pluto_string_new(host, (long)hlen);
    if (__pluto_socket_connect(fd, host_ps, port) < 0) {
        // Covers refusal, unreachability, and connect-phase timeouts alike:
        // the connection never opened, so the request was never sent.
        __pluto_socket_close(fd);
        pluto_boundary_fail(1, "connect failed (nothing sent)");
        return NULL;
    }
    void *nl = __pluto_string_new("\n", 1);
    void *req = __pluto_string_concat(__pluto_string_concat(method_str, nl), payload_str);
    if (__pluto_write_framed(fd, req) < 0) {
        // The frame is length-prefixed and was not fully written, so the
        // server can never read a complete frame and never dispatches.
        __pluto_socket_close(fd);
        pluto_boundary_fail(1, "send failed before the request frame completed (not dispatched)");
        return NULL;
    }
    // Client-side response deadline (rfc-distributed-safety.md names this
    // gap): PLUTO_RPC_TIMEOUT_MS, default 30s, <= 0 disables. Applied only
    // AFTER the request frame went out — a response-wait timeout is the
    // canonical AMBIGUOUS failure and must never be laundered into a
    // definite one.
    {
        long rpc_timeout_ms = 30000;
        const char *env = getenv("PLUTO_RPC_TIMEOUT_MS");
        if (env && *env) rpc_timeout_ms = atol(env);
        if (rpc_timeout_ms > 0) {
            __pluto_socket_set_read_timeout(fd, rpc_timeout_ms);
        }
    }
    void *resp = __pluto_read_framed(fd);
    int timed_out = framed_read_timed_out;
    __pluto_socket_close(fd);
    if (!resp) {
        // The full request frame was handed off and no response returned
        // (reset, EOF, truncated response, or the response deadline
        // elapsed). The effect may or may not have applied — the one honest
        // classification is ambiguity; the timeout is named in the message
        // as diagnostic detail only.
        if (timed_out) {
            pluto_boundary_fail(0, "request sent, no response within deadline (outcome unknown)");
        } else {
            pluto_boundary_fail(0, "request sent, no response (outcome unknown)");
        }
    }
    return resp; // NULL on read failure -> caller raises NetworkError
}

// Parse a pluto string as a long (for primitive remote responses).
long __pluto_parse_long(void *s) {
    const char *data;
    long len;
    __pluto_string_data(s, &data, &len);
    char buf[32];
    long m = len < 31 ? len : 31;
    for (long i = 0; i < m; i++) buf[i] = data[i];
    buf[m] = 0;
    return atol(buf);
}

// ── Serve helpers (generated RPC server side) ─────────────────────────────────
// Bind+listen a TCP socket on 0.0.0.0:<port>. Returns the listener fd, or -1.
/* The serving process's dialable address becomes its entity home, so
 * handles exported from here can be called back (PLUTO_SELF_ADDR overrides
 * the 127.0.0.1 default for multi-host deployments). Must run before the
 * first entity export mints the opaque fallback token. */
void __pluto_serve_set_self_addr(long port) {
    const char *self_addr = getenv("PLUTO_SELF_ADDR");
    if (self_addr && strchr(self_addr, ':')) {
        snprintf(entity_home, sizeof(entity_home), "%s", self_addr);
    } else {
        snprintf(entity_home, sizeof(entity_home), "127.0.0.1:%ld", port);
    }
}

void *__pluto_entity_resolve_local(long id) {
    ENTITY_LOCK();
    void *live = (id >= 1 && id <= entity_count) ? entity_registry[id - 1] : NULL;
    ENTITY_UNLOCK();
    return live;
}

/* Linear JSON string escaping (quote + escape in one pass). The stdlib's
 * per-byte concat loop was quadratic in allocation churn — a 20KB string
 * generated ~200MB of intermediates and hundreds of GC cycles. */
void *__pluto_json_escape_string(void *s) {
    const char *data;
    long len;
    __pluto_string_data(s, &data, &len);
    /* measure */
    long out_len = 2; /* quotes */
    for (long i = 0; i < len; i++) {
        unsigned char b = (unsigned char)data[i];
        if (b == '"' || b == '\\' || b == '\b' || b == '\f' || b == '\n' || b == '\r' || b == '\t') {
            out_len += 2;
        } else if (b < 32) {
            out_len += 6; /* \u00XX */
        } else {
            out_len += 1;
        }
    }
    char *buf = (char *)malloc((size_t)out_len);
    long o = 0;
    buf[o++] = '"';
    static const char hexd[] = "0123456789abcdef";
    for (long i = 0; i < len; i++) {
        unsigned char b = (unsigned char)data[i];
        switch (b) {
        case '"': buf[o++] = '\\'; buf[o++] = '"'; break;
        case '\\': buf[o++] = '\\'; buf[o++] = '\\'; break;
        case '\b': buf[o++] = '\\'; buf[o++] = 'b'; break;
        case '\f': buf[o++] = '\\'; buf[o++] = 'f'; break;
        case '\n': buf[o++] = '\\'; buf[o++] = 'n'; break;
        case '\r': buf[o++] = '\\'; buf[o++] = 'r'; break;
        case '\t': buf[o++] = '\\'; buf[o++] = 't'; break;
        default:
            if (b < 32) {
                buf[o++] = '\\'; buf[o++] = 'u'; buf[o++] = '0'; buf[o++] = '0';
                buf[o++] = hexd[b >> 4]; buf[o++] = hexd[b & 0xf];
            } else {
                buf[o++] = (char)b;
            }
        }
    }
    buf[o++] = '"';
    void *result = __pluto_string_new(buf, o);
    free(buf);
    return result;
}

long __pluto_serve_listen(long port) {
    long fd = __pluto_socket_create(2, 1, 0);
    if (fd < 0) return -1;
    __pluto_socket_set_reuseaddr(fd);
    void *host = __pluto_string_new("0.0.0.0", 7);
    if (__pluto_socket_bind(fd, host, port) < 0) { __pluto_socket_close(fd); return -1; }
    if (__pluto_socket_listen(fd, 128) < 0) { __pluto_socket_close(fd); return -1; }
    // Reap connection-handler children automatically (see __pluto_fork) so they
    // don't accumulate as zombies.
    signal(SIGCHLD, SIG_IGN);
    return fd;
}

// Fork a child to handle one connection concurrently: returns 0 in the child,
// the child pid in the parent. The accept loop forks per connection so a slow
// or stuck client blocks only its own child, not the whole server, and multiple
// clients are served in parallel. Each request is handled in an isolated
// process (copy-on-write): per-request mutations do not persist across requests
// — durable state belongs in an external store.
long __pluto_fork(void) {
    __pluto_gc_prepare_fork();
    long pid = (long)fork();
    __pluto_gc_after_fork(pid == 0);
    return pid;
}

// Exit the current process (used by a handler child after replying). `_exit`
// avoids re-running atexit handlers / flushing the parent's inherited buffers.
void __pluto_process_exit(long code) {
    _exit((int)code);
}

// Accept one connection on the listener, returning the connection fd.
//
// A receive timeout is set on the accepted connection so that a slow or stuck
// client (one that opens a connection but never sends a complete request) can't
// block the single-threaded server indefinitely — head-of-line denial of
// service. On timeout the recv fails, the handler abandons that connection, and
// the server returns to accepting other clients.
long __pluto_serve_accept(long fd) {
    long conn = __pluto_socket_accept(fd);
    if (conn >= 0) {
        struct timeval tv;
        tv.tv_sec = 5;
        tv.tv_usec = 0;
        setsockopt((int)conn, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv));
    }
    return conn;
}

// The actual port a listener is bound to (handy when binding port 0).
long __pluto_serve_port(long fd) {
    return __pluto_socket_get_port(fd);
}

// Return the nth newline-delimited field of `s` (0-based); empty if out of range.
// Used to split a `<method>\n<arg1>\n<arg2>...` request.
// Escape a primitive string for the newline-delimited RPC wire so it carries no
// raw newline (the field delimiter): '\' -> "\\", newline -> "\n". Reversed by
// __pluto_wire_unescape. Struct/enum arguments go through JSON, which already
// escapes newlines, so only primitive string fields need this.
// ── Wire helpers for float/bool/nullable values ─────────────────────────────
// Raw strtod parse for wire decoding (unlike __pluto_string_to_float, which
// returns a boxed nullable for user code).
double __pluto_wire_parse_float(void *s) {
    const char *data;
    long len;
    __pluto_string_data(s, &data, &len);
    char *tmp = (char *)malloc((size_t)len + 1);
    if (!tmp) return 0.0;
    memcpy(tmp, data, (size_t)len);
    tmp[len] = '\0';
    double v = strtod(tmp, NULL);
    free(tmp);
    return v;
}

// Nullable framing: a nullable wire field is "N" (none) or "V<payload>".
// The payload is the inner type's own encoding (already newline-safe).
void *__pluto_wire_opt_wrap(void *payload, long is_none) {
    if (is_none) return __pluto_string_new("N", 1);
    const char *data;
    long len;
    __pluto_string_data(payload, &data, &len);
    char *buf = (char *)malloc((size_t)len + 2);
    if (!buf) return payload;
    buf[0] = 'V';
    memcpy(buf + 1, data, (size_t)len);
    void *r = __pluto_string_new(buf, len + 1);
    free(buf);
    return r;
}

long __pluto_wire_opt_is_none(void *s) {
    const char *data;
    long len;
    __pluto_string_data(s, &data, &len);
    return (len < 1 || data[0] == 'N') ? 1 : 0;
}

void *__pluto_wire_opt_payload(void *s) {
    const char *data;
    long len;
    __pluto_string_data(s, &data, &len);
    if (len < 1) return __pluto_string_new("", 0);
    return __pluto_string_new(data + 1, len - 1);
}

void *__pluto_wire_escape(void *s) {
    const char *data;
    long len;
    __pluto_string_data(s, &data, &len);
    char *buf = (char *)malloc((size_t)(len * 2 + 1));
    if (!buf) return s;
    long j = 0;
    for (long i = 0; i < len; i++) {
        char c = data[i];
        if (c == '\\') { buf[j++] = '\\'; buf[j++] = '\\'; }
        else if (c == '\n') { buf[j++] = '\\'; buf[j++] = 'n'; }
        else { buf[j++] = c; }
    }
    void *r = __pluto_string_new(buf, j);
    free(buf);
    return r;
}

void *__pluto_wire_unescape(void *s) {
    const char *data;
    long len;
    __pluto_string_data(s, &data, &len);
    char *buf = (char *)malloc((size_t)(len + 1));
    if (!buf) return s;
    long j = 0;
    for (long i = 0; i < len; i++) {
        if (data[i] == '\\' && i + 1 < len) {
            char n = data[i + 1];
            if (n == '\\') { buf[j++] = '\\'; i++; }
            else if (n == 'n') { buf[j++] = '\n'; i++; }
            else { buf[j++] = data[i]; }
        } else {
            buf[j++] = data[i];
        }
    }
    void *r = __pluto_string_new(buf, j);
    free(buf);
    return r;
}

void *__pluto_request_field(void *s, long index) {
    const char *data;
    long len;
    __pluto_string_data(s, &data, &len);
    long field = 0;
    long start = 0;
    for (long i = 0; i <= len; i++) {
        if (i == len || data[i] == '\n') {
            if (field == index) return __pluto_string_new(data + start, i - start);
            field++;
            start = i + 1;
        }
    }
    return __pluto_string_new("", 0);
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
        const char *str_data;
        long slen;
        __pluto_string_data(s, &str_data, &slen);
        const unsigned char *data = (const unsigned char *)str_data;
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
//
// Error protocol (issue #367: errno captured at the syscall site):
// - long-returning fns return >= 0 on success and -errno on failure;
// - string/array-returning fns record errno in a thread-local
//   (`__pluto_fs_last_errno`) set to 0 on success, captured immediately
//   after the failing syscall, before control returns to Pluto code. This
//   retires the old read-global-errno-after-return pattern, which any
//   intervening allocation or safepoint could clobber.
// - every blocking syscall is bracketed with GC safe regions so a slow
//   disk operation (an fsync can take hundreds of ms) never stalls
//   stop-the-world. GC-heap access (string/array construction, cstr
//   conversion which may allocate for slices) stays OUTSIDE the brackets;
//   buffers passed into syscalls stay reachable from this frame's stack,
//   which the collector scans conservatively.

static __thread long __pluto_fs_saved_errno = 0;

long __pluto_fs_last_errno(void) {
    return __pluto_fs_saved_errno;
}

void *__pluto_fs_errstr(long code) {
    const char *msg = strerror((int)code);
    return __pluto_string_new(msg, (long)strlen(msg));
}

long __pluto_fs_err_noent(void) {
    return (long)ENOENT;
}

long __pluto_fs_open_read(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    __pluto_gc_enter_safe_region();
    long fd = (long)open(path, O_RDONLY);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    return fd < 0 ? -err : fd;
}

long __pluto_fs_open_write(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    __pluto_gc_enter_safe_region();
    long fd = (long)open(path, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    return fd < 0 ? -err : fd;
}

long __pluto_fs_open_append(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    __pluto_gc_enter_safe_region();
    long fd = (long)open(path, O_WRONLY | O_CREAT | O_APPEND, 0644);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    return fd < 0 ? -err : fd;
}

long __pluto_fs_close(long fd) {
    __pluto_gc_enter_safe_region();
    int rc = close((int)fd);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    return rc == 0 ? 0 : -err;
}

// EOF/error disambiguation (issue #367): "" with last_errno == 0 is EOF;
// "" with last_errno != 0 is a read failure.
void *__pluto_fs_read(long fd, long max_bytes) {
    __pluto_fs_saved_errno = 0;
    if (max_bytes <= 0) return __pluto_string_new("", 0);
    if (max_bytes > 104857600) max_bytes = 104857600; // 100MB cap
    char *buf = (char *)malloc((size_t)max_bytes);
    if (!buf) {
        __pluto_fs_saved_errno = (long)ENOMEM;
        return __pluto_string_new("", 0);
    }
    __pluto_gc_enter_safe_region();
    ssize_t n = read((int)fd, buf, (size_t)max_bytes);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    if (n < 0) {
        free(buf);
        __pluto_fs_saved_errno = err;
        return __pluto_string_new("", 0);
    }
    if (n == 0) {
        free(buf);
        return __pluto_string_new("", 0);
    }
    void *result = __pluto_string_new(buf, n);
    free(buf);
    return result;
}

// Loops to completion (issue #367 short-write fix). Returns the byte count
// written (== len) on success, -errno on the first failing write.
long __pluto_fs_write(long fd, void *data_str) {
    const char *data;
    long len;
    __pluto_string_data(data_str, &data, &len);
    __pluto_gc_enter_safe_region();
    size_t total = 0;
    long err = 0;
    while (total < (size_t)len) {
        ssize_t n = write((int)fd, data + total, (size_t)len - total);
        if (n < 0) {
            if (errno == EINTR) continue;
            err = (long)errno;
            break;
        }
        if (n == 0) { err = (long)EIO; break; }
        total += (size_t)n;
    }
    __pluto_gc_leave_safe_region();
    return err != 0 ? -err : (long)total;
}

// ── Bytes-typed file I/O (issue #368) ─────────────────────────────────────────
// Identical syscall paths and error protocol to the string variants; only the
// handle type at the boundary changes. Reads land in a malloc scratch buffer
// FIRST and the GC handle is allocated after — the syscall blocks in a GC safe
// region, so no GC-visible allocation may be in flight across it.

// Build a bytes handle from a scratch buffer. GC allocation — must be called
// OUTSIDE any safe region.
static long __pluto_fs_bytes_from_scratch(const char *buf, long n) {
    long *handle = (long *)gc_alloc(24, GC_TAG_BYTES, 3);
    long cap = n > 16 ? n : 16;
    unsigned char *data = (unsigned char *)malloc((size_t)cap);
    if (!data) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    if (n > 0) memcpy(data, buf, (size_t)n);
    handle[0] = n;
    handle[1] = cap;
    handle[2] = (long)data;
    return (long)handle;
}

// EOF/error disambiguation matches __pluto_fs_read: empty bytes with
// last_errno == 0 is EOF; empty bytes with last_errno != 0 is a failure.
long __pluto_fs_read_bytes(long fd, long max_bytes) {
    __pluto_fs_saved_errno = 0;
    if (max_bytes <= 0) return __pluto_fs_bytes_from_scratch(NULL, 0);
    if (max_bytes > 104857600) max_bytes = 104857600; // 100MB cap
    char *buf = (char *)malloc((size_t)max_bytes);
    if (!buf) {
        __pluto_fs_saved_errno = (long)ENOMEM;
        return __pluto_fs_bytes_from_scratch(NULL, 0);
    }
    __pluto_gc_enter_safe_region();
    ssize_t n = read((int)fd, buf, (size_t)max_bytes);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    if (n < 0) {
        free(buf);
        __pluto_fs_saved_errno = err;
        return __pluto_fs_bytes_from_scratch(NULL, 0);
    }
    long result = __pluto_fs_bytes_from_scratch(buf, (long)n);
    free(buf);
    return result;
}

// Positioned read (pread): never touches the descriptor's seek offset.
long __pluto_fs_read_at(long fd, long offset, long max_bytes) {
    __pluto_fs_saved_errno = 0;
    if (max_bytes <= 0) return __pluto_fs_bytes_from_scratch(NULL, 0);
    if (max_bytes > 104857600) max_bytes = 104857600; // 100MB cap
    char *buf = (char *)malloc((size_t)max_bytes);
    if (!buf) {
        __pluto_fs_saved_errno = (long)ENOMEM;
        return __pluto_fs_bytes_from_scratch(NULL, 0);
    }
    __pluto_gc_enter_safe_region();
    ssize_t n = pread((int)fd, buf, (size_t)max_bytes, (off_t)offset);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    if (n < 0) {
        free(buf);
        __pluto_fs_saved_errno = err;
        return __pluto_fs_bytes_from_scratch(NULL, 0);
    }
    long result = __pluto_fs_bytes_from_scratch(buf, (long)n);
    free(buf);
    return result;
}

// Loops to completion like __pluto_fs_write. Returns the byte count written
// (== len) on success, -errno on the first failing write. The data buffer is
// malloc memory owned by the handle; the handle stays reachable from this
// frame's stack, which the collector scans conservatively.
long __pluto_fs_write_bytes(long fd, long bytes_handle) {
    long *h = (long *)bytes_handle;
    long len = h[0];
    const unsigned char *data = (const unsigned char *)h[2];
    __pluto_gc_enter_safe_region();
    size_t total = 0;
    long err = 0;
    while (total < (size_t)len) {
        ssize_t n = write((int)fd, data + total, (size_t)len - total);
        if (n < 0) {
            if (errno == EINTR) continue;
            err = (long)errno;
            break;
        }
        if (n == 0) { err = (long)EIO; break; }
        total += (size_t)n;
    }
    __pluto_gc_leave_safe_region();
    return err != 0 ? -err : (long)total;
}

// Positioned write (pwrite): never touches the descriptor's seek offset.
// Loops on short writes, advancing the position with the progress.
long __pluto_fs_write_at(long fd, long offset, long bytes_handle) {
    long *h = (long *)bytes_handle;
    long len = h[0];
    const unsigned char *data = (const unsigned char *)h[2];
    __pluto_gc_enter_safe_region();
    size_t total = 0;
    long err = 0;
    while (total < (size_t)len) {
        ssize_t n = pwrite((int)fd, data + total, (size_t)len - total,
                           (off_t)offset + (off_t)total);
        if (n < 0) {
            if (errno == EINTR) continue;
            err = (long)errno;
            break;
        }
        if (n == 0) { err = (long)EIO; break; }
        total += (size_t)n;
    }
    __pluto_gc_leave_safe_region();
    return err != 0 ? -err : (long)total;
}

// whence_tag: 0 = Start (SEEK_SET), 1 = Current (SEEK_CUR), 2 = End (SEEK_END).
long __pluto_fs_seek(long fd, long offset, long whence_tag) {
    int whence = whence_tag == 0 ? SEEK_SET : (whence_tag == 1 ? SEEK_CUR : SEEK_END);
    __pluto_gc_enter_safe_region();
    off_t result = lseek((int)fd, (off_t)offset, whence);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    return result < 0 ? -err : (long)result;
}

// ── Durability primitives (issue #367) ────────────────────────────────────────
//
// sync/sync_data mean FULL durability on every platform (owner decision D2):
// fsync/fdatasync on Linux, fcntl(F_FULLFSYNC) on Darwin — Darwin's fsync
// explicitly does not flush the drive's volatile cache. ENOTSUP from
// F_FULLFSYNC (SMB/NFS mounts) is returned as an error, never silently
// degraded. Directory sync uses plain fsync (the SQLite convention:
// F_FULLFSYNC is a file-data barrier; a directory's dirty page is the
// name→inode mapping, for which fsync is the portable primitive).
//
// Testing hooks (CI-reachable without real EIO):
// - PLUTO_FS_SYNC_FAIL_AT=<n>: the nth file-sync call fails with EIO
//   without issuing the syscall.
// - __pluto_fs_sync_count(): syncs actually ISSUED (not injected failures),
//   so a test can assert the syscall really happened.

static __thread long __pluto_fs_syncs_issued = 0;
static __thread long __pluto_fs_sync_calls = 0;
static __thread long __pluto_fs_sync_fail_at = -2; // -2 = env not read yet, -1 = disabled

long __pluto_fs_sync_count(void) {
    return __pluto_fs_syncs_issued;
}

static int __pluto_fs_sync_inject_fail(void) {
    if (__pluto_fs_sync_fail_at == -2) {
        const char *v = getenv("PLUTO_FS_SYNC_FAIL_AT");
        __pluto_fs_sync_fail_at = v ? atol(v) : -1;
    }
    if (__pluto_fs_sync_fail_at < 0) return 0;
    __pluto_fs_sync_calls++;
    return __pluto_fs_sync_calls == __pluto_fs_sync_fail_at;
}

// Full-durability sync of a file descriptor; returns 0 or -errno.
static long __pluto_fs_do_sync_fd(long fd) {
    if (__pluto_fs_sync_inject_fail()) return -(long)EIO;
    __pluto_gc_enter_safe_region();
#ifdef __APPLE__
    int rc = fcntl((int)fd, F_FULLFSYNC);
#else
    int rc = fsync((int)fd);
#endif
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    __pluto_fs_syncs_issued++;
    return rc < 0 ? -err : 0;
}

long __pluto_fs_sync(long fd) {
    return __pluto_fs_do_sync_fd(fd);
}

long __pluto_fs_sync_data(long fd) {
    if (__pluto_fs_sync_inject_fail()) return -(long)EIO;
    __pluto_gc_enter_safe_region();
#ifdef __APPLE__
    int rc = fcntl((int)fd, F_FULLFSYNC);
#else
    int rc = fdatasync((int)fd);
#endif
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    __pluto_fs_syncs_issued++;
    return rc < 0 ? -err : 0;
}

// fsync a directory: required for crash-safe rename/create/remove (the
// name→inode mapping is the directory's dirty page, not the file's).
static long __pluto_fs_do_sync_dir(const char *path) {
    __pluto_gc_enter_safe_region();
    int fd = open(path, O_RDONLY);
    long err = (long)errno;
    if (fd >= 0) {
        int rc = fsync(fd);
        err = (long)errno;
        int crc = close(fd);
        long cerr = (long)errno;
        if (rc < 0) {
            __pluto_gc_leave_safe_region();
            return -err;
        }
        if (crc < 0) {
            __pluto_gc_leave_safe_region();
            return -cerr;
        }
        __pluto_gc_leave_safe_region();
        return 0;
    }
    __pluto_gc_leave_safe_region();
    return -err;
}

long __pluto_fs_sync_dir(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    return __pluto_fs_do_sync_dir(path);
}

// Atomic durable replace (issue #367): same-dir temp write → full sync of
// the temp → rename over the target → fsync the parent directory. One
// packaged C fn so intermediate failures always clean up the temp file.
//
// Returns 0 on success, -errno on failure. Failure phase is reported by
// __pluto_fs_replace_phase(): 0 = failed before the rename landed (the old
// file is intact, the temp was cleaned up); 1 = the rename landed but the
// directory sync failed (contents replaced, durability unwarranted).
static __thread long __pluto_fs_replace_phase_v = 0;

long __pluto_fs_replace_phase(void) {
    return __pluto_fs_replace_phase_v;
}

long __pluto_fs_replace_all(void *path_str, void *data_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    const char *data;
    long len;
    __pluto_string_data(data_str, &data, &len);
    __pluto_fs_replace_phase_v = 0;

    size_t plen = strlen(path);
    char *tmp = (char *)malloc(plen + 48);
    if (!tmp) return -(long)ENOMEM;
    static __thread unsigned long replace_seq = 0;
    replace_seq++;
    snprintf(tmp, plen + 48, "%s.tmp.%ld.%lu", path, (long)getpid(), replace_seq);

    // Parent directory for the final fsync ('.' when the path has no '/').
    char *dir = (char *)malloc(plen + 2);
    if (!dir) { free(tmp); return -(long)ENOMEM; }
    const char *slash = strrchr(path, '/');
    if (slash && slash != path) {
        size_t dlen = (size_t)(slash - path);
        memcpy(dir, path, dlen);
        dir[dlen] = '\0';
    } else if (slash == path) {
        strcpy(dir, "/");
    } else {
        strcpy(dir, ".");
    }

    long err = 0;
    __pluto_gc_enter_safe_region();
    int fd = open(tmp, O_WRONLY | O_CREAT | O_EXCL, 0644);
    if (fd < 0) {
        err = -(long)errno;
        __pluto_gc_leave_safe_region();
        free(tmp); free(dir);
        return err;
    }
    size_t total = 0;
    while (total < (size_t)len) {
        ssize_t n = write(fd, data + total, (size_t)len - total);
        if (n < 0) {
            if (errno == EINTR) continue;
            err = -(long)errno;
            break;
        }
        if (n == 0) { err = -(long)EIO; break; }
        total += (size_t)n;
    }
    __pluto_gc_leave_safe_region();
    if (err != 0) {
        __pluto_gc_enter_safe_region();
        close(fd);
        unlink(tmp);
        __pluto_gc_leave_safe_region();
        free(tmp); free(dir);
        return err;
    }

    // Full-durability sync of the temp BEFORE the rename — otherwise the
    // rename can land durably while the contents are still in cache.
    err = __pluto_fs_do_sync_fd((long)fd);
    __pluto_gc_enter_safe_region();
    int crc = close(fd);
    long cerr = (long)errno;
    __pluto_gc_leave_safe_region();
    if (err == 0 && crc < 0) err = -cerr;
    if (err != 0) {
        __pluto_gc_enter_safe_region();
        unlink(tmp);
        __pluto_gc_leave_safe_region();
        free(tmp); free(dir);
        return err;
    }

    __pluto_gc_enter_safe_region();
    int rrc = rename(tmp, path);
    long rerr = (long)errno;
    if (rrc < 0) unlink(tmp);
    __pluto_gc_leave_safe_region();
    if (rrc < 0) {
        free(tmp); free(dir);
        return -rerr;
    }

    // The rename has landed; a directory-sync failure from here on is a
    // durability report about a replace that DID happen.
    err = __pluto_fs_do_sync_dir(dir);
    free(tmp); free(dir);
    if (err != 0) {
        __pluto_fs_replace_phase_v = 1;
        return err;
    }
    return 0;
}

void *__pluto_fs_read_all(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    __pluto_fs_saved_errno = 0;
    __pluto_gc_enter_safe_region();
    int fd = open(path, O_RDONLY);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    if (fd < 0) {
        __pluto_fs_saved_errno = err;
        return __pluto_string_new("", 0);
    }
    struct stat st;
    __pluto_gc_enter_safe_region();
    int src = fstat(fd, &st);
    err = (long)errno;
    __pluto_gc_leave_safe_region();
    if (src != 0) {
        __pluto_gc_enter_safe_region();
        close(fd);
        __pluto_gc_leave_safe_region();
        __pluto_fs_saved_errno = err;
        return __pluto_string_new("", 0);
    }
    size_t size = (size_t)st.st_size;
    char *buf = (char *)malloc(size > 0 ? size : 1);
    if (!buf) {
        __pluto_gc_enter_safe_region();
        close(fd);
        __pluto_gc_leave_safe_region();
        __pluto_fs_saved_errno = (long)ENOMEM;
        return __pluto_string_new("", 0);
    }
    __pluto_gc_enter_safe_region();
    size_t total_read = 0;
    err = 0;
    while (total_read < size) {
        ssize_t n = read(fd, buf + total_read, size - total_read);
        if (n < 0) {
            if (errno == EINTR) continue;
            err = (long)errno;
            break;
        }
        if (n == 0) break; // truncated under us: return what we got
        total_read += (size_t)n;
    }
    int crc = close(fd);
    long cerr = (long)errno;
    __pluto_gc_leave_safe_region();
    if (err == 0 && crc < 0) err = cerr;
    if (err != 0) {
        free(buf);
        __pluto_fs_saved_errno = err;
        return __pluto_string_new("", 0);
    }
    void *result = __pluto_string_new(buf, (long)total_read);
    free(buf);
    return result;
}

// Shared body of write_all/append_all: open with `flags`, write the whole
// buffer, close — surfacing close() failures (issue #367: on NFS-class
// filesystems close is where deferred write errors appear; swallowing it
// reports success for lost data).
static long __pluto_fs_write_whole(const char *path, const char *data, long len, int flags) {
    __pluto_gc_enter_safe_region();
    int fd = open(path, flags, 0644);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    if (fd < 0) return -err;
    __pluto_gc_enter_safe_region();
    size_t total = 0;
    err = 0;
    while (total < (size_t)len) {
        ssize_t n = write(fd, data + total, (size_t)len - total);
        if (n < 0) {
            if (errno == EINTR) continue;
            err = (long)errno;
            break;
        }
        if (n == 0) { err = (long)EIO; break; }
        total += (size_t)n;
    }
    int crc = close(fd);
    long cerr = (long)errno;
    __pluto_gc_leave_safe_region();
    if (err != 0) return -err;
    if (crc < 0) return -cerr;
    return 0;
}

long __pluto_fs_write_all(void *path_str, void *data_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    const char *data;
    long len;
    __pluto_string_data(data_str, &data, &len);
    return __pluto_fs_write_whole(path, data, len, O_WRONLY | O_CREAT | O_TRUNC);
}

long __pluto_fs_append_all(void *path_str, void *data_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    const char *data;
    long len;
    __pluto_string_data(data_str, &data, &len);
    return __pluto_fs_write_whole(path, data, len, O_WRONLY | O_CREAT | O_APPEND);
}

// ── Bytes-typed one-shots (issue #368) ────────────────────────────────────────
// Same open/loop/close-surfacing discipline as the string forms; only the
// handle type at the boundary changes.

// Mirrors __pluto_fs_read_all: errors via last_errno + empty bytes; the GC
// handle is allocated only after every syscall bracket is closed.
long __pluto_fs_read_all_bytes(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    __pluto_fs_saved_errno = 0;
    __pluto_gc_enter_safe_region();
    int fd = open(path, O_RDONLY);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    if (fd < 0) {
        __pluto_fs_saved_errno = err;
        return __pluto_fs_bytes_from_scratch(NULL, 0);
    }
    struct stat st;
    __pluto_gc_enter_safe_region();
    int src = fstat(fd, &st);
    err = (long)errno;
    __pluto_gc_leave_safe_region();
    if (src != 0) {
        __pluto_gc_enter_safe_region();
        close(fd);
        __pluto_gc_leave_safe_region();
        __pluto_fs_saved_errno = err;
        return __pluto_fs_bytes_from_scratch(NULL, 0);
    }
    size_t size = (size_t)st.st_size;
    char *buf = (char *)malloc(size > 0 ? size : 1);
    if (!buf) {
        __pluto_gc_enter_safe_region();
        close(fd);
        __pluto_gc_leave_safe_region();
        __pluto_fs_saved_errno = (long)ENOMEM;
        return __pluto_fs_bytes_from_scratch(NULL, 0);
    }
    __pluto_gc_enter_safe_region();
    size_t total_read = 0;
    err = 0;
    while (total_read < size) {
        ssize_t n = read(fd, buf + total_read, size - total_read);
        if (n < 0) {
            if (errno == EINTR) continue;
            err = (long)errno;
            break;
        }
        if (n == 0) break; // truncated under us: return what we got
        total_read += (size_t)n;
    }
    int crc = close(fd);
    long cerr = (long)errno;
    __pluto_gc_leave_safe_region();
    if (err == 0 && crc < 0) err = cerr;
    if (err != 0) {
        free(buf);
        __pluto_fs_saved_errno = err;
        return __pluto_fs_bytes_from_scratch(NULL, 0);
    }
    long result = __pluto_fs_bytes_from_scratch(buf, (long)total_read);
    free(buf);
    return result;
}

long __pluto_fs_write_all_bytes(void *path_str, long bytes_handle) {
    const char *path = __pluto_string_to_cstr(path_str);
    long *h = (long *)bytes_handle;
    return __pluto_fs_write_whole(path, (const char *)h[2], h[0],
                                  O_WRONLY | O_CREAT | O_TRUNC);
}

long __pluto_fs_append_all_bytes(void *path_str, long bytes_handle) {
    const char *path = __pluto_string_to_cstr(path_str);
    long *h = (long *)bytes_handle;
    return __pluto_fs_write_whole(path, (const char *)h[2], h[0],
                                  O_WRONLY | O_CREAT | O_APPEND);
}

long __pluto_fs_exists(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    struct stat st;
    __pluto_gc_enter_safe_region();
    int rc = stat(path, &st);
    __pluto_gc_leave_safe_region();
    return rc == 0 ? 1 : 0;
}

long __pluto_fs_file_size(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    struct stat st;
    __pluto_gc_enter_safe_region();
    int rc = stat(path, &st);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    if (rc != 0) return -err;
    return (long)st.st_size;
}

// stat as a flat int array: [size, modified_unix_secs, is_dir, is_file,
// mode_bits]. Empty array + last_errno on failure.
void *__pluto_fs_stat(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    __pluto_fs_saved_errno = 0;
    struct stat st;
    __pluto_gc_enter_safe_region();
    int rc = stat(path, &st);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    if (rc != 0) {
        __pluto_fs_saved_errno = err;
        return __pluto_array_new(0);
    }
    void *arr = __pluto_array_new(5);
    __pluto_array_push(arr, (long)st.st_size);
    __pluto_array_push(arr, (long)st.st_mtime);
    __pluto_array_push(arr, S_ISDIR(st.st_mode) ? 1 : 0);
    __pluto_array_push(arr, S_ISREG(st.st_mode) ? 1 : 0);
    __pluto_array_push(arr, (long)(st.st_mode & 07777));
    return arr;
}

long __pluto_fs_is_dir(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    struct stat st;
    __pluto_gc_enter_safe_region();
    int rc = stat(path, &st);
    __pluto_gc_leave_safe_region();
    if (rc != 0) return 0;
    return S_ISDIR(st.st_mode) ? 1 : 0;
}

long __pluto_fs_is_file(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    struct stat st;
    __pluto_gc_enter_safe_region();
    int rc = stat(path, &st);
    __pluto_gc_leave_safe_region();
    if (rc != 0) return 0;
    return S_ISREG(st.st_mode) ? 1 : 0;
}

long __pluto_fs_remove(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    __pluto_gc_enter_safe_region();
    int rc = unlink(path);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    return rc == 0 ? 0 : -err;
}

long __pluto_fs_mkdir(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    __pluto_gc_enter_safe_region();
    int rc = mkdir(path, 0755);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    return rc == 0 ? 0 : -err;
}

// mkdir -p: create every missing component. Existing directories (including
// the full path) are success; a non-directory in the way is an error.
long __pluto_fs_create_dir_all(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    size_t plen = strlen(path);
    if (plen == 0) return -(long)ENOENT;
    char *buf = (char *)malloc(plen + 1);
    if (!buf) return -(long)ENOMEM;
    memcpy(buf, path, plen + 1);
    long result = 0;
    __pluto_gc_enter_safe_region();
    for (size_t i = 1; i <= plen; i++) {
        if (buf[i] != '/' && buf[i] != '\0') continue;
        if (buf[i - 1] == '/') continue; // "//" runs and trailing '/'
        char saved = buf[i];
        buf[i] = '\0';
        if (mkdir(buf, 0755) != 0 && errno != EEXIST) {
            result = -(long)errno;
            buf[i] = saved;
            break;
        }
        buf[i] = saved;
    }
    if (result == 0) {
        struct stat st;
        if (stat(path, &st) != 0) result = -(long)errno;
        else if (!S_ISDIR(st.st_mode)) result = -(long)ENOTDIR;
    }
    __pluto_gc_leave_safe_region();
    free(buf);
    return result;
}

long __pluto_fs_rmdir(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    __pluto_gc_enter_safe_region();
    int rc = rmdir(path);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    return rc == 0 ? 0 : -err;
}

// Recursive delete. Assumes it is called inside a safe region. Depth-first:
// files unlinked, subdirectories recursed then rmdir'd. Symlinks are
// unlinked, never followed (lstat).
static long __pluto_fs_remove_tree(const char *path) {
    struct stat st;
    if (lstat(path, &st) != 0) return -(long)errno;
    if (!S_ISDIR(st.st_mode)) {
        return unlink(path) == 0 ? 0 : -(long)errno;
    }
    DIR *d = opendir(path);
    if (!d) return -(long)errno;
    size_t plen = strlen(path);
    struct dirent *entry;
    long result = 0;
    while (result == 0 && (entry = readdir(d)) != NULL) {
        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0)
            continue;
        size_t nlen = strlen(entry->d_name);
        char *child = (char *)malloc(plen + 1 + nlen + 1);
        if (!child) { result = -(long)ENOMEM; break; }
        memcpy(child, path, plen);
        child[plen] = '/';
        memcpy(child + plen + 1, entry->d_name, nlen + 1);
        result = __pluto_fs_remove_tree(child);
        free(child);
    }
    closedir(d);
    if (result != 0) return result;
    return rmdir(path) == 0 ? 0 : -(long)errno;
}

long __pluto_fs_remove_dir_all(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    // Refusals ("/", "") are enforced in the stdlib too; defense in depth.
    if (path[0] == '\0' || strcmp(path, "/") == 0) return -(long)EINVAL;
    __pluto_gc_enter_safe_region();
    long result = __pluto_fs_remove_tree(path);
    __pluto_gc_leave_safe_region();
    return result;
}

long __pluto_fs_rename(void *from_str, void *to_str) {
    const char *from = __pluto_string_to_cstr(from_str);
    const char *to = __pluto_string_to_cstr(to_str);
    __pluto_gc_enter_safe_region();
    int rc = rename(from, to);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    return rc == 0 ? 0 : -err;
}

long __pluto_fs_copy(void *from_str, void *to_str) {
    const char *from = __pluto_string_to_cstr(from_str);
    const char *to = __pluto_string_to_cstr(to_str);
    __pluto_gc_enter_safe_region();
    long err = 0;
    int src_fd = open(from, O_RDONLY);
    if (src_fd < 0) {
        err = -(long)errno;
        __pluto_gc_leave_safe_region();
        return err;
    }
    int dst_fd = open(to, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (dst_fd < 0) {
        err = -(long)errno;
        close(src_fd);
        __pluto_gc_leave_safe_region();
        return err;
    }
    char buf[4096];
    ssize_t n;
    while ((n = read(src_fd, buf, sizeof(buf))) > 0) {
        size_t written = 0;
        while (written < (size_t)n) {
            ssize_t w = write(dst_fd, buf + written, (size_t)n - written);
            if (w < 0) {
                if (errno == EINTR) continue;
                err = -(long)errno;
                break;
            }
            if (w == 0) { err = -(long)EIO; break; }
            written += (size_t)w;
        }
        if (err != 0) break;
    }
    if (err == 0 && n < 0) err = -(long)errno;
    // Close errors surfaced (issue #367): deferred write errors can appear
    // at close; the first error wins but both fds are always released.
    int src_crc = close(src_fd);
    long src_cerr = (long)errno;
    int dst_crc = close(dst_fd);
    long dst_cerr = (long)errno;
    if (err == 0 && dst_crc < 0) err = -dst_cerr;
    if (err == 0 && src_crc < 0) err = -src_cerr;
    __pluto_gc_leave_safe_region();
    return err;
}

void *__pluto_fs_list_dir(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    __pluto_fs_saved_errno = 0;
    // Collect names into a malloc'd buffer inside the safe region, then
    // leave it before building GC strings.
    __pluto_gc_enter_safe_region();
    DIR *d = opendir(path);
    long err = (long)errno;
    if (!d) {
        __pluto_gc_leave_safe_region();
        __pluto_fs_saved_errno = err;
        return __pluto_array_new(0);
    }
    size_t cap = 4096, used = 0, count = 0;
    char *names = (char *)malloc(cap);
    struct dirent *entry;
    while (names && (entry = readdir(d)) != NULL) {
        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0)
            continue;
        size_t nlen = strlen(entry->d_name) + 1;
        if (used + nlen > cap) {
            cap = (used + nlen) * 2;
            char *grown = (char *)realloc(names, cap);
            if (!grown) { free(names); names = NULL; break; }
            names = grown;
        }
        memcpy(names + used, entry->d_name, nlen);
        used += nlen;
        count++;
    }
    closedir(d);
    __pluto_gc_leave_safe_region();
    if (!names) {
        __pluto_fs_saved_errno = (long)ENOMEM;
        return __pluto_array_new(0);
    }
    void *arr = __pluto_array_new((long)(count > 0 ? count : 1));
    size_t off = 0;
    for (size_t i = 0; i < count; i++) {
        size_t nlen = strlen(names + off);
        void *name_str = __pluto_string_new(names + off, (long)nlen);
        __pluto_array_push(arr, (long)name_str);
        off += nlen + 1;
    }
    free(names);
    return arr;
}

void *__pluto_fs_temp_dir(void) {
    __pluto_fs_saved_errno = 0;
    char tmpl[] = "/tmp/pluto_XXXXXX";
    __pluto_gc_enter_safe_region();
    char *result = mkdtemp(tmpl);
    long err = (long)errno;
    __pluto_gc_leave_safe_region();
    if (!result) {
        __pluto_fs_saved_errno = err;
        return __pluto_string_new("", 0);
    }
    return __pluto_string_new(tmpl, (long)strlen(tmpl));
}

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
        const char *data_a, *data_e;
        long len_a, len_e;
        __pluto_string_data(actual, &data_a, &len_a);
        __pluto_string_data(expected, &data_e, &len_e);
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
    const char *data;
    long len;
    __pluto_string_data(name_str, &data, &len);
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
    const char *src;
    long slen;
    __pluto_string_data(pluto_str, &src, &slen);
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



void *__pluto_remote_request(void *service_str, void *method_str, void *payload_str) {
    return pluto_boundary_request("PLUTO_REMOTE_", service_str, method_str, payload_str);
}

/* `at` placement boundaries resolve their physical plan from
 * PLUTO_DOMAIN_<SERVICE>: bound -> socket transport, unbound -> the caller
 * uses its colocated instance (docs/design/rfc-at-placement.md). */
void *__pluto_domain_request(void *service_str, void *method_str, void *payload_str) {
    return pluto_boundary_request("PLUTO_DOMAIN_", service_str, method_str, payload_str);
}

long __pluto_domain_bound(void *service_str) {
    const char *svc;
    long svclen;
    __pluto_string_data(service_str, &svc, &svclen);
    char envname[256];
    const char *prefix = "PLUTO_DOMAIN_";
    int n = 0;
    while (prefix[n]) { envname[n] = prefix[n]; n++; }
    for (long i = 0; i < svclen && n < (int)sizeof(envname) - 1; i++) {
        char c = svc[i];
        if (c >= 'a' && c <= 'z') c -= 32;
        envname[n++] = c;
    }
    envname[n] = 0;
    return getenv(envname) != NULL ? 1 : 0;
}
