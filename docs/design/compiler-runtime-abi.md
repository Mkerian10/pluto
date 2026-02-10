# Compiler Runtime ABI

This document describes the C runtime surface that the compiler currently targets.
It is meant to be a practical reference for contributors working on codegen or
runtime changes.

## Calling Convention

The compiler lowers Pluto values to native values as follows:

- `int` -> `i64`
- `float` -> `f64`
- `bool` -> `i8` in the compiler, widened to `i32` when calling C print
- `string` -> pointer to a GC-managed string header
- `class` -> pointer to GC-managed heap-allocated struct data
- `array` -> pointer to a GC-managed array handle
- `Map<K,V>` -> pointer to a GC-managed hash table
- `Set<T>` -> pointer to a GC-managed hash table
- `byte` -> `i8` (unsigned 8-bit value, zero-extended to `i64` at C boundaries)
- `bytes` -> pointer to a GC-managed bytes handle
- `fn(...)` -> pointer to a GC-managed closure object `[fn_ptr, captures...]`
- `trait` -> currently passed as a fat pointer (data + vtable) at parameter
  boundaries only

## Print Functions

Implemented in `runtime/builtins.c` and imported by name.

- `__pluto_print_int(long value)`
- `__pluto_print_float(double value)`
- `__pluto_print_string(void *header)`
- `__pluto_print_bool(int value)`

`__pluto_print_string` expects a Pluto string header (see below).

## Allocation

- `__pluto_alloc(long size) -> void *`

The compiler uses this for class instance allocation.

## String Runtime

String layout is:

- 8-byte header: length as `long`
- Immediately followed by UTF-8 bytes
- Null terminator at the end (`len` + 1)

Runtime functions:

- `__pluto_string_new(const char *data, long len) -> void *`
- `__pluto_string_concat(void *a, void *b) -> void *`
- `__pluto_string_eq(void *a, void *b) -> int` (1 or 0)
- `__pluto_string_len(void *s) -> long`
- `__pluto_string_contains(void *s, void *needle) -> long` (1 or 0)
- `__pluto_string_starts_with(void *s, void *prefix) -> long` (1 or 0)
- `__pluto_string_ends_with(void *s, void *suffix) -> long` (1 or 0)
- `__pluto_string_index_of(void *s, void *needle) -> long` (-1 if not found)
- `__pluto_string_substring(void *s, long start, long len) -> void *`
- `__pluto_string_trim(void *s) -> void *`
- `__pluto_string_to_upper(void *s) -> void *`
- `__pluto_string_to_lower(void *s) -> void *`
- `__pluto_string_replace(void *s, void *old, void *new) -> void *`
- `__pluto_string_split(void *s, void *delim) -> void *` (returns array handle)
- `__pluto_string_char_at(void *s, long index) -> void *` (aborts on OOB)
- `__pluto_string_to_bytes(void *s) -> void *` (returns bytes handle)

## Array Runtime

Array handle layout (24 bytes):

- `len: long`
- `cap: long`
- `data_ptr: long *` (points to `len`/`cap` sized storage)

Runtime functions:

- `__pluto_array_new(long cap) -> void *`
- `__pluto_array_push(void *handle, long value)`
- `__pluto_array_get(void *handle, long index) -> long`
- `__pluto_array_set(void *handle, long index, long value)`
- `__pluto_array_len(void *handle) -> long`

The compiler stores array elements as `i64` slots. Floats are bitcast and bools
are zero-extended before storage.

## Bytes Runtime

Bytes handle layout (24 bytes):

- `len: long`
- `cap: long`
- `data_ptr: unsigned char *` (points to packed byte storage, 1 byte per element)

Runtime functions:

- `__pluto_bytes_new() -> void *`
- `__pluto_bytes_push(void *handle, long value)` — stores `(unsigned char)(value & 0xFF)`, grows 2x when full
- `__pluto_bytes_get(void *handle, long index) -> long` — zero-extends u8 to i64, aborts on OOB
- `__pluto_bytes_set(void *handle, long index, long value)` — stores `(unsigned char)(value & 0xFF)`, aborts on OOB
- `__pluto_bytes_len(void *handle) -> long`
- `__pluto_bytes_to_string(void *handle) -> void *` — raw byte copy, no UTF-8 validation
- `__pluto_string_to_bytes(void *str) -> void *` — copies string bytes into new bytes buffer

Unlike arrays (which store `i64` slots), bytes store packed `unsigned char` values
(1 byte per element). This is 8x more memory-efficient for binary data.

**Platform assumption:** All runtime functions use `long` parameters/returns.
On LP64 platforms (aarch64-apple-darwin, x86_64-linux), `long` = 64 bits = matches
Cranelift I64. Portability to LLP64 platforms (Windows) would require switching to
fixed-width `int64_t`.

## Map Runtime

Maps are GC-managed open-addressing hash tables (GC tag 4).

Runtime functions:

- `__pluto_map_new() -> void *`
- `__pluto_map_insert(void *map, long key, long value)`
- `__pluto_map_get(void *map, long key) -> long`
- `__pluto_map_contains(void *map, long key) -> int`
- `__pluto_map_remove(void *map, long key)`
- `__pluto_map_len(void *map) -> long`
- `__pluto_map_keys(void *map) -> void *` (returns array handle)
- `__pluto_map_values(void *map) -> void *` (returns array handle)

Keys and values are stored as `i64` slots (same bitcasting as arrays).

## Set Runtime

Sets are GC-managed open-addressing hash tables (GC tag 5).

Runtime functions:

- `__pluto_set_new() -> void *`
- `__pluto_set_insert(void *set, long element)`
- `__pluto_set_contains(void *set, long element) -> int`
- `__pluto_set_remove(void *set, long element)`
- `__pluto_set_len(void *set) -> long`
- `__pluto_set_to_array(void *set) -> void *` (returns array handle)

## Error Runtime

Runtime functions for error propagation:

- `__pluto_set_error(long error_tag, void *error_data)`
- `__pluto_get_error() -> long` (returns error tag, 0 = no error)
- `__pluto_clear_error()`
- `__pluto_get_error_data() -> void *`

Error tags are assigned per-error-type by the compiler. The runtime uses thread-local storage to hold the current error state.

## Garbage Collector

The runtime includes a mark-and-sweep GC. All heap allocations go through `__pluto_alloc` which registers them with the GC.

GC tags identify allocation types for tracing:

| Tag | Type | Tracing behavior |
|-----|------|-----------------|
| 0 | Unknown/class | Traces all pointer-sized slots |
| 1 | String | No internal references to trace |
| 2 | Array | Traces all element slots |
| 3 | Closure | Traces captured variable slots |
| 4 | Map | Traces key/value slots |
| 5 | Set | Traces element slots |
| 6 | JSON | Traces child value slots |
| 7 | Task | Traces result/error/closure slots |
| 8 | Bytes | No internal references to trace |

GC functions:

- `__pluto_gc_push_root(void *ptr)` — push a GC root onto the shadow stack
- `__pluto_gc_pop_roots(long count)` — pop N roots from the shadow stack
- `__pluto_gc_heap_size() -> long` — return current heap usage in bytes

The compiler generates `push_root`/`pop_roots` calls around allocations that might trigger collection.

## Trait Runtime (Experimental)

There is a helper for trait fat pointers:

- `__pluto_trait_wrap(long data_ptr, long vtable_ptr) -> void *`

The current compiler does not use this helper. Trait values are represented as
separate data and vtable values at call boundaries, and vtable values are not
carried through locals or return values yet.
