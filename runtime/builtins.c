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
// - Contract enforcement (__pluto_invariant_violation)
// - RPC response parsing (JSON extraction)
//──────────────────────────────────────────────────────────────────────────────

#include "builtins.h"

// ── Print functions ───────────────────────────────────────────────────────────

void __pluto_print_int(long value) {
    printf("%ld\n", value);
}

void __pluto_print_float(double value) {
    printf("%f\n", value);
}

void __pluto_print_string(void *header) {
    const char *data;
    long len;
    __pluto_string_data(header, &data, &len);
    printf("%.*s\n", (int)len, data);
}

void __pluto_print_bool(int value) {
    printf("%s\n", value ? "true" : "false");
}

void __pluto_print_string_no_newline(void *header) {
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
    int len = snprintf(NULL, 0, "%g", value);
    size_t alloc_size = 8 + len + 1;
    void *header = gc_alloc(alloc_size, GC_TAG_STRING, 0);
    *(long *)header = len;
    snprintf((char *)header + 8, len + 1, "%g", value);
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
    return (long)accept((int)fd, (struct sockaddr *)&client_addr, &client_len);
}

long __pluto_socket_connect(long fd, void *host_str, long port) {
    const char *host = __pluto_string_to_cstr(host_str);
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
    const char *data;
    long len;
    __pluto_string_data(data_str, &data, &len);
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

void *__pluto_fs_strerror(void) {
    const char *msg = strerror(errno);
    long len = (long)strlen(msg);
    return __pluto_string_new(msg, len);
}

long __pluto_fs_open_read(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    return (long)open(path, O_RDONLY);
}

long __pluto_fs_open_write(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    return (long)open(path, O_WRONLY | O_CREAT | O_TRUNC, 0644);
}

long __pluto_fs_open_append(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
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
    const char *data;
    long len;
    __pluto_string_data(data_str, &data, &len);
    ssize_t written = write((int)fd, data, (size_t)len);
    return (long)written;
}

long __pluto_fs_seek(long fd, long offset, long whence) {
    off_t result = lseek((int)fd, (off_t)offset, (int)whence);
    return (long)result;
}

void *__pluto_fs_read_all(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
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
    const char *path = __pluto_string_to_cstr(path_str);
    const char *data;
    long len;
    __pluto_string_data(data_str, &data, &len);
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
    const char *path = __pluto_string_to_cstr(path_str);
    const char *data;
    long len;
    __pluto_string_data(data_str, &data, &len);
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
    const char *path = __pluto_string_to_cstr(path_str);
    struct stat st;
    return stat(path, &st) == 0 ? 1 : 0;
}

long __pluto_fs_file_size(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    struct stat st;
    if (stat(path, &st) != 0) return -1;
    return (long)st.st_size;
}

long __pluto_fs_is_dir(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    struct stat st;
    if (stat(path, &st) != 0) return 0;
    return S_ISDIR(st.st_mode) ? 1 : 0;
}

long __pluto_fs_is_file(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    struct stat st;
    if (stat(path, &st) != 0) return 0;
    return S_ISREG(st.st_mode) ? 1 : 0;
}

long __pluto_fs_remove(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    return unlink(path) == 0 ? 0 : -1;
}

long __pluto_fs_mkdir(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    return mkdir(path, 0755) == 0 ? 0 : -1;
}

long __pluto_fs_rmdir(void *path_str) {
    const char *path = __pluto_string_to_cstr(path_str);
    return rmdir(path) == 0 ? 0 : -1;
}

long __pluto_fs_rename(void *from_str, void *to_str) {
    const char *from = __pluto_string_to_cstr(from_str);
    const char *to = __pluto_string_to_cstr(to_str);
    return rename(from, to) == 0 ? 0 : -1;
}

long __pluto_fs_copy(void *from_str, void *to_str) {
    const char *from = __pluto_string_to_cstr(from_str);
    const char *to = __pluto_string_to_cstr(to_str);
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
    const char *path = __pluto_string_to_cstr(path_str);
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

