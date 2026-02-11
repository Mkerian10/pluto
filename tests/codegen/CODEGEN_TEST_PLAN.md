# Codegen Test Plan: Exhaustive Coverage

**Agent 4: Codegen Explorer**
**Goal:** Find every possible codegen bug through systematic testing
**Approach:** Test every type × every operation × every edge case

## Test Categories (Target: 500+ tests)

### Category 1: Type Representation (80 tests)

**All PlutoType variants must correctly map to Cranelift types:**

1. **Primitives (20 tests)**
   - Int: Zero, positive, negative, i64::MIN, i64::MAX
   - Float: Zero, positive, negative, infinity, NaN, denormal, f64::MIN, f64::MAX
   - Bool: true, false
   - Byte: 0x00, 0xFF, all 256 values (sampled)
   - Void: Functions returning void, void expressions

2. **Strings (10 tests)**
   - Empty string
   - ASCII string
   - Unicode string (emoji, CJK)
   - Very long string (10KB, 1MB)
   - String with null bytes
   - String interpolation result

3. **Classes (15 tests)**
   - Empty class (zero fields)
   - Class with one field (int, float, string, class)
   - Class with multiple fields (mixed types)
   - Class with 100+ fields
   - Nested classes (5 levels deep)
   - Class with bracket deps
   - Class with methods
   - Class implementing traits

4. **Arrays (10 tests)**
   - Empty array
   - Array of ints (1, 10, 1000 elements)
   - Array of floats
   - Array of bools
   - Array of strings
   - Array of classes
   - Array of arrays (nested)
   - Array of nullable types

5. **Enums (10 tests)**
   - Unit variant enum
   - Data-carrying variant enum
   - Enum with mixed variants
   - Enum with 20+ variants
   - Nested enums
   - Enum as match discriminant

6. **Closures/Functions (10 tests)**
   - Zero-parameter closure
   - Multi-parameter closure
   - Closure capturing 0, 1, 5, 20 variables
   - Closure returning closure
   - Calling closure through variable
   - Closure as struct field

7. **Maps & Sets (10 tests)**
   - Empty map, empty set
   - Map<int, int>, Map<string, int>, Map<int, string>
   - Set<int>, Set<string>, Set<enum>
   - Large maps/sets (1000+ entries)
   - Nested maps (Map<int, Map<int, int>>)

8. **Tasks & Channels (5 tests)**
   - Task<int>, Task<string>, Task<void>
   - Sender<int>, Receiver<int>
   - Channel operations

9. **Nullable (5 tests)**
   - int?, string?, class?
   - none literal
   - Nullable boxing (primitive types)

10. **Errors (5 tests)**
    - Error type representation
    - Error value storage
    - Error state in TLS

### Category 2: Arithmetic Operations (60 tests)

**All binary ops × all types:**

1. **Integer arithmetic (20 tests)**
   - Add: 0+0, 1+1, MAX+1 (overflow), MIN-1 (underflow)
   - Sub: 5-3, 0-0, MIN-MAX
   - Mul: 2*3, 0*MAX, MAX*2 (overflow)
   - Div: 6/2, 5/2 (truncation), x/0 (undefined), MIN/-1 (overflow)
   - Mod: 7%3, -7%3, 7%-3, x%0 (undefined)
   - Associativity: (a+b)+c vs a+(b+c)
   - Large expressions: ((((a+b)*c)/d)%e)

2. **Float arithmetic (20 tests)**
   - Add: 1.0+2.0, infinity+1.0, NaN+1.0
   - Sub: 3.0-1.5, infinity-infinity (NaN), 0.0-0.0
   - Mul: 2.5*4.0, infinity*0 (NaN), MAX*2 (infinity)
   - Div: 6.0/2.0, 1.0/0.0 (infinity), 0.0/0.0 (NaN)
   - Precision: 0.1+0.2 != 0.3, denormal handling
   - Special: infinity, -infinity, NaN propagation

3. **Bitwise operations (10 tests)**
   - And: 0b1010 & 0b1100 = 0b1000
   - Or: 0b1010 | 0b0011 = 0b1011
   - Xor: 0b1010 ^ 0b1100 = 0b0110
   - Not: ~0b1010
   - Shift left: 1 << 63 (overflow), 1 << 64 (undefined)
   - Shift right: 8 >> 2, -8 >> 2 (arithmetic shift)

4. **Comparison operations (10 tests)**
   - Int: <, <=, >, >=, ==, !=
   - Float: NaN comparisons, infinity comparisons, -0.0 == 0.0
   - String: equality, inequality
   - Bool: equality

### Category 3: Memory Layout & Alignment (40 tests)

**Verify struct layout matches expected ABI:**

1. **Struct field layout (20 tests)**
   - Single field: int, float, string, class
   - Two fields: (int, int), (int, float), (float, float)
   - Three fields: (int, int, int), (int, float, string)
   - Mixed size fields: (byte, int, byte) — test padding
   - Large structs: 50 fields, 100 fields
   - Nested structs: (struct A { x: int, y: struct B { z: int } })
   - Zero-sized structs (if allowed)

2. **Alignment requirements (10 tests)**
   - Byte alignment (1-byte)
   - Int alignment (8-byte)
   - Float alignment (8-byte)
   - Pointer alignment (8-byte)
   - Struct alignment (max of field alignments)
   - Array alignment

3. **Field access (10 tests)**
   - Read first field, middle field, last field
   - Write first field, middle field, last field
   - Read/write after GC (check pointers still valid)
   - Read 100th field in large struct

### Category 4: Function Calls & Calling Conventions (50 tests)

**Verify all calling conventions work correctly:**

1. **Direct function calls (15 tests)**
   - Zero parameters
   - One parameter (int, float, string, class)
   - Multiple parameters (2, 5, 10, 20)
   - Mixed parameter types
   - Return void, int, float, string, class
   - Recursive calls (factorial, fibonacci)
   - Mutually recursive functions (even/odd)

2. **Method calls (10 tests)**
   - Instance method (self parameter)
   - Method with extra parameters
   - Method returning self
   - Chained method calls (a.b().c().d())
   - Virtual method calls (trait methods)

3. **Closure calls (15 tests)**
   - Call closure with 0, 1, 5 captures
   - Closure returned from function
   - Closure stored in struct
   - Closure as function parameter
   - Closure calling closure
   - Indirect calls through function pointer

4. **Tail calls (5 tests)**
   - Tail recursive function (should optimize?)
   - Non-tail recursive function
   - Mutual tail recursion

5. **Stack depth (5 tests)**
   - Deep recursion (100 levels, 1000 levels)
   - Stack overflow detection

### Category 5: Control Flow (40 tests)

**All control flow constructs:**

1. **If/else (10 tests)**
   - Simple if
   - If with else
   - Nested if (5 levels)
   - If in loop
   - Empty if block, empty else block
   - If as expression (value-producing)

2. **Loops (15 tests)**
   - While true (with break)
   - While with condition
   - For loop (range)
   - Nested loops (2, 3, 5 levels)
   - Loop with continue
   - Loop with break
   - Early loop exit
   - Infinite loop with break in middle
   - Loop with 10,000 iterations

3. **Match (10 tests)**
   - Match on enum (unit variants)
   - Match on enum (data-carrying variants)
   - Match with guards (if supported)
   - Nested match
   - Match returning values
   - Exhaustive match checking

4. **Returns (5 tests)**
   - Early return
   - Multiple return paths
   - Return from nested block
   - Return in loop
   - Return void vs value

### Category 6: Error Handling (30 tests)

**All error operations:**

1. **Raise (5 tests)**
   - Raise builtin error (MathError, IOError)
   - Raise custom error
   - Raise in function
   - Raise in method
   - Raise in closure

2. **Propagate (!) (10 tests)**
   - Propagate from function call
   - Propagate chain (a()! + b()! + c()!)
   - Propagate in arithmetic expression
   - Propagate in nested call
   - Propagate with value unwrap

3. **Catch (10 tests)**
   - Catch specific error type
   - Catch multiple error types
   - Catch in variable
   - Nested catch
   - Catch with fallback value
   - Wildcard catch

4. **Error state management (5 tests)**
   - TLS error state isolation
   - Error state across function calls
   - Error state in spawn/task
   - Error cleared after catch

### Category 7: Concurrency (25 tests)

**spawn, tasks, channels:**

1. **Spawn (10 tests)**
   - Spawn returning int, float, string, void
   - Spawn with 0, 1, 5 captured variables
   - Spawn calling spawn (nested tasks)
   - Spawn 100 tasks concurrently

2. **Task.get() (5 tests)**
   - get() on completed task
   - get() blocking on running task
   - get()! propagating error from task
   - get() catch handling task error

3. **Channels (10 tests)**
   - send/recv non-blocking
   - send/recv blocking
   - send on full channel
   - recv on empty channel
   - Channel iteration
   - Multiple senders, one receiver
   - One sender, multiple receivers

### Category 8: GC Integration (30 tests)

**Memory allocation and collection:**

1. **Allocations (15 tests)**
   - Allocate string
   - Allocate class instance
   - Allocate array
   - Allocate closure
   - Allocate 1,000 objects
   - Allocate 100,000 objects (trigger GC)

2. **GC correctness (10 tests)**
   - Object reachable through local variable (not collected)
   - Object unreachable (should be collected)
   - Object reachable through array (not collected)
   - Object reachable through class field (not collected)
   - Circular references (A -> B -> A)

3. **GC tags (5 tests)**
   - Verify tag on string
   - Verify tag on class
   - Verify tag on array
   - Verify tag on map/set
   - Verify tag on task

### Category 9: Dependency Injection (15 tests)

**DI code generation:**

1. **Bracket deps (5 tests)**
   - Class with one bracket dep
   - Class with multiple bracket deps
   - Nested bracket deps (A[b: B], B[c: C])

2. **App main (5 tests)**
   - Synthetic main generation
   - Singleton allocation
   - Singleton wiring
   - Call to app main

3. **Scoped instances (5 tests)**
   - Scoped class instantiation
   - Scoped singleton injection
   - Scope block cleanup

### Category 10: Contracts (20 tests)

**Runtime contract checking:**

1. **Invariants (10 tests)**
   - Invariant checked after construction
   - Invariant checked after mut method
   - Invariant violation → abort
   - Read lock for non-mut methods
   - Write lock for mut methods

2. **Requires/Ensures (10 tests)**
   - Requires checked on entry
   - Ensures checked on exit
   - old() snapshot values
   - Multiple requires/ensures
   - Violation → abort

### Category 11: Nullable Types (15 tests)

**Nullable codegen:**

1. **Boxing (5 tests)**
   - int? boxed to heap
   - float? boxed to heap
   - bool? boxed to heap
   - string? uses pointer directly

2. **None (5 tests)**
   - none literal = 0
   - Check if value is none
   - Early return on none (?)

3. **Unwrap (?) (5 tests)**
   - Unwrap non-null value
   - Unwrap null → early return
   - Chain unwraps

### Category 12: Edge Cases & Stress Tests (50 tests)

**Find boundary bugs:**

1. **Numeric limits (10 tests)**
   - i64::MIN, i64::MAX
   - f64::MIN, f64::MAX
   - Overflow detection
   - Underflow detection

2. **Large data structures (10 tests)**
   - Array with 1 million elements
   - String with 1MB content
   - Class with 1000 fields
   - Deep nesting (100 levels)

3. **Corner cases (10 tests)**
   - Empty array operations
   - Division by zero behavior
   - Null pointer dereference (shouldn't happen)
   - Stack overflow

4. **Boundary conditions (10 tests)**
   - Zero-length strings
   - Zero-element arrays
   - Zero-field structs
   - Zero-parameter functions

5. **Special values (10 tests)**
   - Float: +0.0 vs -0.0
   - Float: infinity, -infinity, NaN
   - Int: -1 (all bits set)
   - Bool: values other than 0/1

### Category 13: Codegen Correctness (30 tests)

**Verify generated IR is correct:**

1. **Type conversions (10 tests)**
   - int to float (as operator)
   - float to int (truncation)
   - int to bool (0 = false, else true)
   - bool to int

2. **Constant folding (10 tests)**
   - 2+3 should be 5 at compile time
   - true && false should be false
   - Constant expressions in arrays

3. **Dead code elimination (5 tests)**
   - if (false) { ... } should be eliminated
   - Unreachable code after return

4. **Register allocation (5 tests)**
   - Heavy register pressure (20+ live variables)
   - Spill to stack

### Category 14: ABI Compliance (20 tests)

**Interop with C runtime:**

1. **C calling convention (10 tests)**
   - Call C function from Pluto
   - Pass int, float, pointer to C
   - Return int, float, pointer from C
   - Struct passing (if supported)

2. **Stack alignment (5 tests)**
   - Verify stack 16-byte aligned before C calls
   - Verify stack restored after C calls

3. **Calling Pluto from C (5 tests)**
   - C calls Pluto function
   - Pass parameters from C to Pluto
   - Return values from Pluto to C

### Category 15: Platform-Specific (10 tests)

**Architecture-specific codegen:**

1. **AArch64 (5 tests)**
   - Verify correct instruction selection
   - Verify register usage
   - Verify stack frame layout

2. **x86_64 (5 tests)**
   - Same as AArch64

## Testing Strategy

### Phase 1: Write All Tests (Don't Run Yet)

1. Create test file for each category (15 files)
2. Write comprehensive tests in each file
3. Use consistent naming: `test_<category>_<specific_case>`
4. Add comments explaining what each test validates

### Phase 2: Run Tests and Find Bugs

1. Run all tests
2. Document every failure
3. Categorize bugs: P0 (crash/segfault), P1 (wrong behavior), P2 (optimization)
4. Create minimal reproduction for each bug

### Phase 3: Fix Critical Bugs

1. Fix all P0 bugs (zero tolerance for crashes)
2. Fix P1 bugs that block other tests
3. Document remaining P1/P2 bugs

## Success Metrics

- **Target:** 500+ tests written
- **Coverage:** 90%+ of `src/codegen/` code
- **Pass Rate:** 85%+ tests passing
- **Crashes:** 0 segfaults, 0 panics in codegen
- **Memory:** 0 valgrind errors
- **Bugs:** Find 20+ codegen bugs

## Sources

Research based on:
- [Rust Compiler Codegen Testing](https://rustc-dev-guide.rust-lang.org/tests/codegen-backend-tests/intro.html)
- [LLVM Testing Infrastructure](https://rocm.docs.amd.com/projects/llvm-project/en/latest/LLVM/llvm/html/TestingGuide.html)
- [ABI Compliance Testing](https://doc.rust-lang.org/beta/nightly-rustc/rustc_abi/index.html)
- [Calling Conventions](https://www.agner.org/optimize/calling_conventions.pdf)
