// ═══════════════════════════════════════════════════════════════════════════
// Pluto Runtime — Compression (std.compress, issue #371)
// ═══════════════════════════════════════════════════════════════════════════
//
// Thin wrappers over the vendored miniz DEFLATE/INFLATE core (runtime/miniz.c)
// operating on the Pluto `bytes` handle representation. Every wrapper takes
// bytes handles (and ints) and returns a FRESH bytes handle.
//
// Two codecs, one-shot only (streaming is a documented follow-up):
//   • "deflate" — raw DEFLATE (RFC 1951), no framing. The primitive.
//   • "gzip"    — a gzip member (RFC 1952): a 10-byte header, a raw DEFLATE
//                 body, and an 8-byte trailer (CRC-32 + ISIZE, little-endian)
//                 assembled here by hand (miniz has no gzip framing of its own).
//
// ERROR DOCTRINE: compression of valid input cannot fail (the level is range-
// checked in Pluto; OOM traps, like everywhere in the runtime). DECOMPRESSION
// of attacker/wire-controlled input CAN fail — corrupt stream, truncated
// trailer, a CRC/size that doesn't check out, or output that would exceed the
// caller's max_size cap. Those are CONDITIONS, never traps: the wrapper sets a
// thread-local error flag and returns an empty bytes handle, and the
// std.compress Pluto layer reads __pluto_compress_last_error() and raises
// CompressError. A trap (abort) on bad input would make every program that
// decompresses untrusted bytes crashable — exactly backwards.
//
// OUTPUT CAP: decompression never grows unbounded. It inflates into a single
// flat buffer of max_size via tinfl_decompress_mem_to_mem() with the
// non-wrapping-output flag; a stream that expands past max_size simply fails
// the decode (a decompression bomb is rejected, not absorbed). The buffer is
// malloc'd but, thanks to lazy paging, only the pages actually written become
// resident, so a generous default cap is cheap for small payloads. The default
// cap (std.compress.default_max_size) is 64 MiB.

#include "builtins.h"
#include "miniz.h"

// Decompression outcome, read by the Pluto wrappers immediately after a
// decompress call. 0 = success, non-zero = the decode failed and the returned
// bytes handle is a meaningless empty placeholder. Thread-local so concurrent
// tasks can't clobber each other's status (and __thread is legal in both the
// normal and PLUTO_TEST_MODE builds — builtins.c uses it throughout).
static __thread long compress_err = 0;

long __pluto_compress_last_error(void) {
    return compress_err;
}

// Build a fresh GC bytes handle holding a copy of `buf[0..n)`. Mirrors the
// layout of __pluto_bytes_new / __pluto_fs_bytes_from_scratch in builtins.c:
// a 24-byte handle [len][cap][data_ptr] tagged GC_TAG_BYTES, whose data
// buffer comes from the GC backend (see __pluto_gc_buf_new in builtins.h).
static long bytes_from_raw(const unsigned char *buf, long n) {
    long *handle = (long *)gc_alloc(24, GC_TAG_BYTES, 3);
    long cap = n > 16 ? n : 16;
    unsigned char *data = (unsigned char *)__pluto_gc_buf_new(handle, cap);
    if (n > 0 && buf) memcpy(data, buf, (size_t)n);
    PLUTO_GC_SET_BUF(handle, 2, data);
    handle[1] = cap;
    handle[0] = n;
    return (long)handle;
}

// Read the (len, data) view out of a bytes handle.
static void bytes_view(long handle, const unsigned char **data, long *len) {
    long *h = (long *)handle;
    *len = h[0];
    *data = (const unsigned char *)h[2];
}

// Raw DEFLATE of `in_data[0..in_len)` at `level` (0..9). miniz's heap helper
// allocates and sizes the output for us. On any failure returns NULL. The
// `level`→flags mapping uses window_bits <= 0 so NO zlib header is written —
// the body is bare DEFLATE, which is what both the "deflate" codec and the
// gzip framing want.
static unsigned char *raw_deflate(const unsigned char *in_data, long in_len,
                                  long level, size_t *out_len) {
    mz_uint flags = tdefl_create_comp_flags_from_zip_params(
        (int)level, -15 /* raw, no zlib header */, MZ_DEFAULT_STRATEGY);
    return (unsigned char *)tdefl_compress_mem_to_heap(
        in_data, (size_t)in_len, out_len, (int)flags);
}

// Raw INFLATE of `in_data[0..in_len)` into a caller-sized cap buffer. Returns a
// malloc'd buffer (caller frees) with *out_len set, or NULL on failure or if
// the output would exceed max_size. A single flat non-wrapping buffer is the
// cap: tinfl writes at most max_size bytes and fails if it needs more.
static unsigned char *raw_inflate_capped(const unsigned char *in_data, long in_len,
                                         long max_size, size_t *out_len) {
    if (max_size < 0) return NULL;
    // malloc(0) is implementation-defined; give the 0-cap case a 1-byte buffer
    // so the pointer is valid. An empty stream decompresses to 0 bytes anyway.
    size_t cap = (size_t)(max_size > 0 ? max_size : 1);
    unsigned char *out = (unsigned char *)malloc(cap);
    if (!out) { fprintf(stderr, "pluto: out of memory\n"); exit(1); }
    size_t n = tinfl_decompress_mem_to_mem(
        out, (size_t)max_size, in_data, (size_t)in_len,
        TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF);
    if (n == TINFL_DECOMPRESS_MEM_TO_MEM_FAILED) {
        free(out);
        return NULL;
    }
    *out_len = n;
    return out;
}

// Little-endian 32-bit store/load for the gzip trailer.
static void put_u32_le(unsigned char *p, mz_uint32 v) {
    p[0] = (unsigned char)(v & 0xFF);
    p[1] = (unsigned char)((v >> 8) & 0xFF);
    p[2] = (unsigned char)((v >> 16) & 0xFF);
    p[3] = (unsigned char)((v >> 24) & 0xFF);
}
static mz_uint32 get_u32_le(const unsigned char *p) {
    return (mz_uint32)p[0] | ((mz_uint32)p[1] << 8)
         | ((mz_uint32)p[2] << 16) | ((mz_uint32)p[3] << 24);
}

// ── deflate (raw RFC 1951) ────────────────────────────────────────────────

long __pluto_deflate_compress(long in_handle, long level) {
    compress_err = 0;
    const unsigned char *in_data; long in_len;
    bytes_view(in_handle, &in_data, &in_len);
    size_t out_len = 0;
    unsigned char *out = raw_deflate(in_data, in_len, level, &out_len);
    if (!out) { compress_err = 1; return bytes_from_raw(NULL, 0); }
    long r = bytes_from_raw(out, (long)out_len);
    mz_free(out);
    return r;
}

long __pluto_deflate_decompress(long in_handle, long max_size) {
    compress_err = 0;
    const unsigned char *in_data; long in_len;
    bytes_view(in_handle, &in_data, &in_len);
    size_t out_len = 0;
    unsigned char *out = raw_inflate_capped(in_data, in_len, max_size, &out_len);
    if (!out) { compress_err = 1; return bytes_from_raw(NULL, 0); }
    long r = bytes_from_raw(out, (long)out_len);
    free(out);
    return r;
}

// ── gzip (RFC 1952 member around a raw DEFLATE body) ──────────────────────

#define PLUTO_GZIP_HEADER_LEN 10
#define PLUTO_GZIP_TRAILER_LEN 8

long __pluto_gzip_compress(long in_handle, long level) {
    compress_err = 0;
    const unsigned char *in_data; long in_len;
    bytes_view(in_handle, &in_data, &in_len);

    size_t body_len = 0;
    unsigned char *body = raw_deflate(in_data, in_len, level, &body_len);
    if (!body) { compress_err = 1; return bytes_from_raw(NULL, 0); }

    long total = PLUTO_GZIP_HEADER_LEN + (long)body_len + PLUTO_GZIP_TRAILER_LEN;
    unsigned char *out = (unsigned char *)malloc((size_t)total);
    if (!out) { mz_free(body); fprintf(stderr, "pluto: out of memory\n"); exit(1); }

    // Fixed 10-byte header: magic, CM=deflate(8), no flags, mtime=0, xfl=0,
    // OS=255 (unknown). This is the minimal, portable gzip header.
    out[0] = 0x1F; out[1] = 0x8B; out[2] = 0x08; out[3] = 0x00;
    out[4] = 0x00; out[5] = 0x00; out[6] = 0x00; out[7] = 0x00;
    out[8] = 0x00; out[9] = 0xFF;
    memcpy(out + PLUTO_GZIP_HEADER_LEN, body, body_len);

    // Trailer: CRC-32 of the ORIGINAL data, then ISIZE = original length mod 2^32.
    mz_ulong crc = mz_crc32(MZ_CRC32_INIT, in_data, (size_t)in_len);
    unsigned char *trailer = out + PLUTO_GZIP_HEADER_LEN + body_len;
    put_u32_le(trailer, (mz_uint32)crc);
    put_u32_le(trailer + 4, (mz_uint32)((mz_uint64)in_len & 0xFFFFFFFFu));

    mz_free(body);
    long r = bytes_from_raw(out, total);
    free(out);
    return r;
}

long __pluto_gzip_decompress(long in_handle, long max_size) {
    compress_err = 0;
    const unsigned char *in_data; long in_len;
    bytes_view(in_handle, &in_data, &in_len);

    // Smallest possible member: 10 header + (>=2 body) + 8 trailer.
    if (in_len < PLUTO_GZIP_HEADER_LEN + PLUTO_GZIP_TRAILER_LEN
        || in_data[0] != 0x1F || in_data[1] != 0x8B || in_data[2] != 0x08) {
        compress_err = 1; return bytes_from_raw(NULL, 0);
    }
    unsigned char flg = in_data[3];
    // Bit 5, 6, 7 are reserved and MUST be zero (RFC 1952 §2.3.1).
    if (flg & 0xE0) { compress_err = 1; return bytes_from_raw(NULL, 0); }

    long pos = PLUTO_GZIP_HEADER_LEN;
    long body_end = in_len - PLUTO_GZIP_TRAILER_LEN;  // trailer is the last 8 bytes

    // FEXTRA (bit 2): 2-byte little-endian length then that many bytes.
    if (flg & 0x04) {
        if (pos + 2 > body_end) { compress_err = 1; return bytes_from_raw(NULL, 0); }
        long xlen = (long)in_data[pos] | ((long)in_data[pos + 1] << 8);
        pos += 2 + xlen;
    }
    // FNAME (bit 3): NUL-terminated string.
    if (flg & 0x08) {
        while (pos < body_end && in_data[pos] != 0) pos++;
        pos++;  // consume the NUL
    }
    // FCOMMENT (bit 4): NUL-terminated string.
    if (flg & 0x10) {
        while (pos < body_end && in_data[pos] != 0) pos++;
        pos++;  // consume the NUL
    }
    // FHCRC (bit 1): 2-byte header CRC (not verified — it's advisory).
    if (flg & 0x02) {
        pos += 2;
    }
    if (pos < 0 || pos > body_end) { compress_err = 1; return bytes_from_raw(NULL, 0); }

    long body_len = body_end - pos;
    size_t out_len = 0;
    unsigned char *out = raw_inflate_capped(in_data + pos, body_len, max_size, &out_len);
    if (!out) { compress_err = 1; return bytes_from_raw(NULL, 0); }

    // Verify the trailer: CRC-32 and ISIZE (mod 2^32) of the decoded output.
    const unsigned char *trailer = in_data + body_end;
    mz_uint32 want_crc = get_u32_le(trailer);
    mz_uint32 want_isize = get_u32_le(trailer + 4);
    mz_ulong got_crc = mz_crc32(MZ_CRC32_INIT, out, out_len);
    if ((mz_uint32)got_crc != want_crc
        || (mz_uint32)((mz_uint64)out_len & 0xFFFFFFFFu) != want_isize) {
        free(out);
        compress_err = 1;
        return bytes_from_raw(NULL, 0);
    }

    long r = bytes_from_raw(out, (long)out_len);
    free(out);
    return r;
}
