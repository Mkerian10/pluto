# Runtime Architecture

## Overview

The Pluto runtime is written in C and split into three modules for modularity, testability, and maintainability. The total runtime is ~4700 lines of C code.

## Module Structure

```
runtime/
├── builtins.h (126 lines)    — Shared declarations, GC tags, forward decls
├── gc.c (834 lines)          — Garbage collector (mark & sweep, STW)
├── threading.c (2056 lines)  — Concurrency (tasks, channels, select, fibers)
└── builtins.c (1786 lines)   — Core runtime (strings, arrays, I/O, maps, sets)
```

## Module Dependencies

```
builtins.c  ──calls──► gc.c::__pluto_alloc()
threading.c ──calls──► gc.c::__pluto_alloc()
threading.c ──calls──► gc.c::__pluto_gc_register_thread_stack()
```

**Key principle:** GC has no dependencies. Other modules depend on GC for allocation.

## gc.c — Garbage Collector

**Responsibility:** Memory management and garbage collection.

**Algorithm:** Conservative mark-and-sweep with interval tables for pointer lookup.

**Key features:**
- Generational collection threshold (starts at 256 KB, grows 2x after each GC)
- Stop-the-world coordination via safepoint polling (production mode)
- Cooperative fiber scanning (test mode)
- Thread stack registration for concurrent GC root scanning

**Public API:**
- `void __pluto_gc_init(void *stack_bottom)` — Initialize GC with main thread stack
- `void __pluto_gc_collect(void)` — Trigger a GC cycle
- `void *__pluto_alloc(long size)` — Allocate GC-managed memory for user code
- `void __pluto_safepoint(void)` — Check for pending GC and yield if requested
- `void *gc_alloc(size_t size, uint8_t tag, uint16_t field_count)` — Internal allocation (used by runtime modules)
- `size_t __pluto_gc_bytes_allocated(void)` — Query current heap size

**Test mode API:**
- `void __pluto_gc_register_fiber_stack(char *base, size_t size)` — Register fiber stack for scanning
- `void __pluto_gc_mark_fiber_complete(int fiber_id)` — Mark fiber as completed (no longer scan)
- `void __pluto_gc_set_current_fiber(int fiber_id)` — Update currently running fiber
- `void __pluto_gc_enable_fiber_scanning(void)` — Enable fiber stack scanning
- `void __pluto_gc_disable_fiber_scanning(void)` — Disable fiber stack scanning
- `void __pluto_gc_maybe_collect(void)` — Check threshold and collect if needed

**Production mode API:**
- `void __pluto_gc_register_thread_stack(void *lo, void *hi)` — Register spawned task thread
- `void __pluto_gc_deregister_thread_stack(void)` — Deregister thread on task completion
- `void __pluto_gc_task_start(void)` — Increment active task counter (suppresses GC)
- `void __pluto_gc_task_end(void)` — Decrement active task counter
- `int __pluto_gc_check_safepoint(void)` — Check if GC is requesting STW

**Internal data structures:**
- `GCHeader` — 16-byte header on all GC objects (next ptr, size, mark bit, type tag, field count)
- `gc_head` — Linked list of all live objects
- `gc_intervals` — Sorted interval table for fast pointer-to-header lookup
- `gc_worklist` — Stack of objects to mark (DFS marking)
- `gc_thread_stacks[]` — Thread stack bounds for root scanning (production mode)
- `gc_fiber_stacks` — Fiber stack metadata (test mode)

**GC tags** (in `builtins.h`):
```c
#define GC_TAG_OBJECT 0   // class, enum, closure, error, DI singleton
#define GC_TAG_STRING 1   // no child pointers
#define GC_TAG_ARRAY  2   // [len][cap][data_ptr]
#define GC_TAG_TRAIT  3   // [data_ptr][vtable_ptr]
#define GC_TAG_MAP    4   // [count][cap][keys_ptr][vals_ptr][meta_ptr]
#define GC_TAG_SET    5   // [count][cap][keys_ptr][meta_ptr]
#define GC_TAG_JSON   6   // (reserved, unused)
#define GC_TAG_TASK   7   // [closure][result][error][done][sync_ptr]
#define GC_TAG_BYTES  8   // [len][cap][data_ptr]
#define GC_TAG_CHANNEL 9  // [sync_ptr][buf_ptr][capacity][count][head][tail][closed]
```

## threading.c — Concurrency Primitives

**Responsibility:** Task-based concurrency (spawn, channels, select).

**Design:** Dual-mode runtime — test mode uses cooperative fibers with exhaustive DPOR state exploration, production mode uses pthreads.

**Key features:**
- Deep copy semantics for spawn arguments (value isolation between tasks)
- Rwlock synchronization for contract enforcement on shared objects
- Channel operations (send/receive/try_send/try_receive/close/iteration)
- Select API for waiting on multiple channels
- Fiber scheduler with deterministic execution order (test mode)
- Pthread-based tasks with mutex-protected channels (production mode)

**Public API:**

*Task operations:*
- `long __pluto_task_spawn(long closure_ptr)` — Spawn a task, returns Task handle
- `long __pluto_task_get(long task_ptr)` — Block until task completes, return result
- `void __pluto_task_detach(long task_ptr)` — Detach task (no longer joinable)
- `int __pluto_task_is_done(long task_ptr)` — Check if task has completed

*Channel operations:*
- `long __pluto_chan_create(long capacity)` — Create buffered channel
- `void __pluto_chan_send(long chan_ptr, long value)` — Blocking send
- `long __pluto_chan_recv(long chan_ptr)` — Blocking receive
- `int __pluto_chan_try_send(long chan_ptr, long value)` — Non-blocking send (returns 0 on success, 1 if full)
- `void __pluto_chan_close(long chan_ptr)` — Close channel
- `int __pluto_chan_is_closed(long chan_ptr)` — Check if channel is closed
- `long __pluto_chan_sender(long chan_ptr)` — Get Sender handle
- `long __pluto_chan_receiver(long chan_ptr)` — Get Receiver handle

*Select operations:*
- `long __pluto_select_init(long count)` — Create select state for N channels
- `void __pluto_select_add(long select_ptr, long chan_ptr, long idx)` — Register channel
- `long __pluto_select_wait(long select_ptr)` — Block until any channel is ready

*Synchronization (for contracts):*
- `long __pluto_rwlock_new(void)` — Create read-write lock
- `void __pluto_rwlock_read(long lock_ptr)` — Acquire read lock
- `void __pluto_rwlock_write(long lock_ptr)` — Acquire write lock
- `void __pluto_rwlock_unlock_read(long lock_ptr)` — Release read lock
- `void __pluto_rwlock_unlock_write(long lock_ptr)` — Release write lock

*Deep copy (for spawn isolation):*
- `long __pluto_deep_copy(long value, long type_id)` — Deep copy a value

**Test mode specifics:**
- Fiber scheduler with `setcontext`/`swapcontext` for cooperative multitasking
- Exhaustive DPOR state exploration for deterministic testing
- `MAX_FIBERS` = 256
- `FIBER_STACK_SIZE` = 64 KB per fiber

**Production mode specifics:**
- `pthread_create` for spawned tasks
- Mutex-protected channels with condition variables
- Thread-local storage for error state and task handles

## builtins.c — Core Runtime Utilities

**Responsibility:** Everything that isn't GC or concurrency.

**Contents:**
- Print functions (stdout output for all types)
- String operations (allocation, concatenation, slicing, parsing, manipulation)
- Array operations (dynamic arrays with push/pop/get/set/length/reverse/etc.)
- Bytes operations (byte array manipulation)
- Error handling (TLS error state: `__pluto_current_error`)
- Map/Set operations (open-addressing hash tables)
- File I/O (read, write, exists, delete, directory operations)
- Socket I/O (TCP client/server, UDP)
- HTTP client (simple GET/POST via sockets)
- Math builtins (abs, min, max, pow, sqrt, floor, ceil, round, trig functions)
- Test framework (expect assertions)
- Contract enforcement (`__pluto_invariant_violation` aborts on contract failure)
- RPC response parsing (JSON extraction)

**Public API examples:**
- `void __pluto_print_int(long value)`
- `void *__pluto_string_new(const char *data, long len)`
- `void *__pluto_string_concat(void *a, void *b)`
- `void *__pluto_array_new(long capacity)`
- `void __pluto_array_push(void *handle, long value)`
- `long __pluto_array_len(void *handle)`
- `void __pluto_raise_error(void *error_obj)`
- `long __pluto_map_create(void)`
- `void __pluto_map_insert(long map_ptr, long key, long value)`
- `long __pluto_time_ns(void)` — Nanosecond timestamp

## builtins.h — Shared Declarations

**Responsibility:** Declarations shared across all runtime modules.

**Contents:**
- System includes (`stdio.h`, `stdlib.h`, `pthread.h`, etc.)
- GC tag definitions (`GC_TAG_OBJECT`, `GC_TAG_STRING`, etc.)
- Thread-local storage declarations (`__pluto_current_error`, `__pluto_current_task`)
- `GCHeader` struct definition
- `ChannelSync` struct (production mode only)
- Forward declarations for cross-module functions
- All public GC API declarations
- Test mode vs production mode conditional compilation (`#ifdef PLUTO_TEST_MODE`)

## Build Process

The compiler (`src/lib.rs`) compiles all three C files separately and links them together:

```rust
fn compile_runtime_object(test_mode: bool) -> Result<PathBuf, CompileError> {
    // Load sources via include_str!
    let gc_src = include_str!("../runtime/gc.c");
    let threading_src = include_str!("../runtime/threading.c");
    let builtins_src = include_str!("../runtime/builtins.c");
    let header_src = include_str!("../runtime/builtins.h");

    // Write to temp directory
    // Compile: cc -c -I<dir> gc.c -o gc.o
    // Compile: cc -c -I<dir> threading.c -o threading.o
    // Compile: cc -c -I<dir> builtins.c -o builtins.o
    // Link: ld -r gc.o threading.o builtins.o -o runtime.o

    // Returns path to runtime.o
}
```

**Test mode compilation:** Add `-DPLUTO_TEST_MODE -Wno-deprecated-declarations` flags.

**Production mode compilation:** Add `-pthread` on Linux.

The resulting `runtime.o` is then linked with the Cranelift-generated object code to produce the final executable.

## Testing

**Unit tests:** None (runtime is tested via integration tests only).

**Integration tests:** 530+ tests in `tests/integration/` exercise the runtime through compiled Pluto programs.

**Test mode:** Enables deterministic testing via fiber scheduler and DPOR. All concurrency tests run in test mode.

**Production mode:** Used for normal compilation and execution.

## Performance Characteristics

**GC:**
- Allocation: O(1) with occasional O(n) for GC pause
- Collection: O(heap size) for mark phase, O(live objects) for sweep
- Threshold starts at 256 KB, doubles after each GC

**Maps/Sets:**
- Insert/lookup: O(1) average, O(n) worst case (open addressing)
- Resize: O(n) when load factor > 0.7

**Arrays:**
- Push: O(1) amortized (doubles capacity when full)
- Index: O(1)
- Pop: O(1)

**Channels:**
- Send/receive on buffered channel: O(1) when not blocking
- Select on N channels: O(N) to check all channels

## String Slices

String-returning operations (`substring`, `trim`, `trim_start`, `trim_end`, `split`) return
lightweight 24-byte **slices** instead of copying data. A slice is a GC-tracked view into an
existing owned string:

```
Owned string:  [len: i64][data bytes...][null terminator]   GC_TAG_STRING
String slice:  [backing_ptr: i64][offset: i64][len: i64]    GC_TAG_STRING_SLICE
```

**Key properties:**
- All string-consuming functions (`eq`, `concat`, `contains`, `print`, etc.) accept both owned
  strings and slices transparently via `__pluto_string_data()`, which dispatches on the GC tag.
- Slice-of-slice is flattened: creating a slice of a slice points back to the original backing
  string (no reference chains).
- Slices are **materialized** (copied to an owned string) at escape boundaries in codegen:
  function returns, struct/enum field stores, array/map/set inserts, and closure captures.
  This prevents large backing strings from being kept alive by small slices.
- The GC traces the backing pointer (`field_count=1`), keeping the backing string alive as long
  as any slice references it.
- Empty slices (len == 0) are returned as empty owned strings.
- C interop functions that need null-terminated strings use `__pluto_string_to_cstr()`, which
  materializes slices on demand.

## Future Optimizations

Potential improvements (not yet implemented):
- Generational GC (young/old generation split)
- Work-stealing scheduler for parallelism
- Lock-free channels
- Incremental marking
- Compacting GC to reduce fragmentation
- Arena allocation for short-lived objects

## Debugging

**Environment variables:**
- `PLUTO_TEST_MODE=1` — Enable fiber scheduler (set via `-DPLUTO_TEST_MODE` at compile time)
- No runtime debug flags yet (future: `PLUTO_GC_DEBUG`, `PLUTO_TRACE_ALLOC`, etc.)

**Logging:**
- GC prints to stderr on collection (if manually enabled in gc.c)
- Contract violations print to stderr and abort
- Test framework prints assertion failures to stdout

## See Also

- `docs/design/concurrency.md` — Concurrency model deep dive
- `docs/design/memory-model.md` — Memory layout and GC details
- `docs/design/contracts.md` — Runtime contract enforcement
- `CLAUDE.md` — Build and test commands
