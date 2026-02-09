#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <unistd.h>
#include <signal.h>

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

// String utility functions

void *__pluto_string_substring(void *s, long start, long len) {
    long slen = *(long *)s;
    const char *data = (const char *)s + 8;
    // Clamp start and len to valid range
    if (start < 0) start = 0;
    if (start > slen) start = slen;
    if (len < 0) len = 0;
    if (start + len > slen) len = slen - start;
    void *header = malloc(8 + len + 1);
    if (!header) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
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
    void *header = malloc(8 + newlen + 1);
    if (!header) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    *(long *)header = newlen;
    memcpy((char *)header + 8, data + start, newlen);
    ((char *)header)[8 + newlen] = '\0';
    return header;
}

void *__pluto_string_to_upper(void *s) {
    long slen = *(long *)s;
    const char *data = (const char *)s + 8;
    void *header = malloc(8 + slen + 1);
    if (!header) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
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
    void *header = malloc(8 + slen + 1);
    if (!header) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
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
    // Empty old → return copy unchanged
    if (olen == 0) {
        void *header = malloc(8 + slen + 1);
        if (!header) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
        *(long *)header = slen;
        memcpy((char *)header + 8, sdata, slen);
        ((char *)header)[8 + slen] = '\0';
        return header;
    }
    // Count occurrences
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
    void *header = malloc(8 + newlen + 1);
    if (!header) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
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
        // Split into individual characters
        for (long i = 0; i < slen; i++) {
            void *ch = malloc(8 + 1 + 1);
            if (!ch) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
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
            // Last segment
            void *seg = malloc(8 + remaining + 1);
            if (!seg) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
            *(long *)seg = remaining;
            memcpy((char *)seg + 8, p, remaining);
            ((char *)seg)[8 + remaining] = '\0';
            __pluto_array_push(arr, (long)seg);
            break;
        }
        const char *found = (const char *)memmem(p, remaining, ddata, dlen);
        if (!found) {
            void *seg = malloc(8 + remaining + 1);
            if (!seg) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
            *(long *)seg = remaining;
            memcpy((char *)seg + 8, p, remaining);
            ((char *)seg)[8 + remaining] = '\0';
            __pluto_array_push(arr, (long)seg);
            break;
        }
        long seglen = found - p;
        void *seg = malloc(8 + seglen + 1);
        if (!seg) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
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
    void *header = malloc(8 + 1 + 1);
    if (!header) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    *(long *)header = 1;
    ((char *)header)[8] = data[index];
    ((char *)header)[9] = '\0';
    return header;
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

// Error handling runtime — thread-local error state (single-threaded for MVP)
static void *__pluto_current_error = NULL;

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

// Socket runtime — POSIX sockets for networking

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
