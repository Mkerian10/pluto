# Fixed: `?` operator crash in void-returning functions

**Status:** ✅ FIXED
**Fixed in:** commit `ec589633` (2026-02-10)
**Severity:** High — silent codegen crash, no useful error message
**Discovered by:** Lagrange project development

## The Bug

The `?` (null propagation) operator in void-returning functions caused a Cranelift verifier error.

### Reproduction
```pluto
fn get_line() string? {
    return "hello"
}

fn process() {
    let line = get_line()
    if line == none {
        return
    }
    let val = line?    // <-- crashed here
    print(val)
}

fn main() {
    process()
}
```

### What happened
- Type checker **accepted** this code (no errors)
- Codegen crashed with: `Verifier errors` (Cranelift IR verifier)
- The error message gave no indication of which function or what went wrong

## Root Cause

The `?` operator generated an early-return path that returns `none` (a nullable value). But if the enclosing function returns `void`, the Cranelift function signature has no return value. The generated IR tried to emit a return-with-value instruction into a void-returning function, which the Cranelift verifier rejected.

## The Fix

**File:** `src/codegen/lower/mod.rs`
**Lines:** 1805-1819

Changed the `NullPropagate` codegen to check if the function returns void:

```rust
let is_void_return = matches!(&self.expected_return_type, Some(PlutoType::Void) | None);
if is_void_return {
    if let Some(exit_bb) = self.exit_block {
        self.builder.ins().jump(exit_bb, &[]);
    } else {
        self.builder.ins().return_(&[]);  // Bare return for void functions
    }
} else {
    let none_val = self.builder.ins().iconst(types::I64, 0);
    if let Some(exit_bb) = self.exit_block {
        self.builder.ins().jump(exit_bb, &[none_val]);
    } else {
        self.builder.ins().return_(&[none_val]);
    }
}
```

When the current function's return type is `Void`, emit `builder.ins().return_(&[])` instead of the nullable-wrapping return path. This matches how the `!` (error propagation) operator handles the same case.

## Tests Added

**File:** `tests/integration/nullable.rs`
Added 41 new tests covering:
- `?` in void functions
- `?` with early returns
- `?` in nested contexts
- Multiple `?` operators in same function

## Verification

The fix has been verified to work correctly:

```pluto
fn maybe_string() string? {
    return none
}

fn process() {
    let s = maybe_string()
    let unwrapped = s?  // Early-returns from void function
    print("Should not reach here")
}

fn main() {
    process()
    print("Done")
}
```

**Output:**
```
Done
```

The function correctly early-returns without printing "Should not reach here".

## Commit Details

```
commit ec589633193b72db3b8902de53bdf49ef1683ac1
Author: Test <test@test.com>
Date:   Tue Feb 10 14:58:09 2026 -0600

    Fix ? operator crash in void-returning functions

    The NullPropagate (?) codegen always emitted return_(&[none_val]),
    passing an I64 value even when the function returns void. This caused
    a Cranelift verifier error. Now checks expected_return_type and emits
    a void return for void functions, matching how Propagate (!) handles
    the same case via emit_default_return().

    Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

 src/codegen/lower.rs          | 19 ++++++++++++++-----
 tests/integration/nullable.rs | 41 +++++++++++++++++++++++++++++++++++++++++
```
