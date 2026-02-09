# Compiler Runtime ABI

This document describes the C runtime surface that the compiler currently targets.
It is meant to be a practical reference for contributors working on codegen or
runtime changes.

## Calling Convention

The compiler lowers Pluto values to native values as follows:

- `int` -> `i64`
- `float` -> `f64`
- `bool` -> `i8` in the compiler, widened to `i32` when calling C print
- `string` -> pointer to a string header
- `class` -> pointer to heap-allocated struct data
- `array` -> pointer to an array handle
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

## Trait Runtime (Experimental)

There is a helper for trait fat pointers:

- `__pluto_trait_wrap(long data_ptr, long vtable_ptr) -> void *`

The current compiler does not use this helper. Trait values are represented as
separate data and vtable values at call boundaries, and vtable values are not
carried through locals or return values yet.
