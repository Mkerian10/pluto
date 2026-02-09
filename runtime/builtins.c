#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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

void *__pluto_alloc(long size) {
    if (size == 0) size = 8;
    void *ptr = calloc(1, size);
    if (!ptr) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    return ptr;
}

void *__pluto_trait_wrap(long data_ptr, long vtable_ptr) {
    long *handle = (long *)malloc(16);
    if (!handle) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    handle[0] = data_ptr;
    handle[1] = vtable_ptr;
    return handle;
}

void *__pluto_string_new(const char *data, long len) {
    void *header = malloc(8 + len + 1);
    if (!header) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    *(long *)header = len;
    memcpy((char *)header + 8, data, len);
    ((char *)header)[8 + len] = '\0';
    return header;
}

void *__pluto_string_concat(void *a, void *b) {
    long len_a = *(long *)a;
    long len_b = *(long *)b;
    long total = len_a + len_b;
    void *header = malloc(8 + total + 1);
    if (!header) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
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

// Array runtime functions
// Handle layout (24 bytes): [len: long] [cap: long] [data_ptr: long*]

void *__pluto_array_new(long cap) {
    long *handle = (long *)malloc(24);
    if (!handle) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    handle[0] = 0;   // len
    handle[1] = cap;  // cap
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

void *__pluto_int_to_string(long value) {
    int len = snprintf(NULL, 0, "%ld", value);
    void *header = malloc(8 + len + 1);
    if (!header) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    *(long *)header = len;
    snprintf((char *)header + 8, len + 1, "%ld", value);
    return header;
}

void *__pluto_float_to_string(double value) {
    int len = snprintf(NULL, 0, "%f", value);
    void *header = malloc(8 + len + 1);
    if (!header) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    *(long *)header = len;
    snprintf((char *)header + 8, len + 1, "%f", value);
    return header;
}

void *__pluto_bool_to_string(int value) {
    const char *s = value ? "true" : "false";
    long len = value ? 4 : 5;
    void *header = malloc(8 + len + 1);
    if (!header) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    *(long *)header = len;
    memcpy((char *)header + 8, s, len);
    ((char *)header)[8 + len] = '\0';
    return header;
}
