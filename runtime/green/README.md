# Green-task context switch (#369 two-tier scheduler)

The register-only cooperative context switch the green tier is built on. Saves
only the callee-saved registers to the current stack, swaps the stack pointer,
restores, and returns into the resumed context. No syscall (unlike `ucontext`
`swapcontext`, which does a `sigprocmask` per switch), no heap.

    void pluto_ctx_swap(void **from_sp, void *to_sp);
    // saves the current context (sp) into *from_sp, resumes the context at to_sp

- `ctx_arm64.s` — AAPCS64 (x19-x28, fp/lr, d8-d15). Run-verified on aarch64 macOS.
- `ctx_amd64.s` — x86-64 SysV (rbx/rbp/r12-r15). Assembles; **run-test on x86-64
  before the scheduler depends on it.**

## Priming a fresh fiber

A new fiber's stack is primed so the first `pluto_ctx_swap` into it "returns"
to the entry function on a clean stack:

- **arm64**: reserve a 160-byte saved-register frame at a 16-aligned `base`
  (= stacktop - 160); write the entry address into the `lr` slot at `base+88`;
  the fiber's saved sp = `base`. First swap restores (zeroed) regs, `add sp,#160`,
  `ret` -> entry with sp = stacktop.
- **amd64**: reserve `[r15..rbp]` (48 bytes) + an 8-byte return slot above them
  at 16-aligned `base`; write the entry address into the return slot at
  `base+48`; saved sp = `base`. First swap pops the 6 regs, `ret` -> entry.

## Benchmark

    cc -O2 -o /tmp/ctx_bench ctx_bench.c ctx_arm64.s && /tmp/ctx_bench

Measured aarch64 macOS (2026-10-09), round-trip (two switches):

| mechanism | round-trip | vs asm |
|---|---|---|
| **pluto_ctx_swap (this)** | **~30 ns** | 1x |
| ucontext swapcontext | ~885 ns | ~28x |
| pthread condvar rendezvous | ~1800 ns | ~58x |
| pthread create+join | ~10200 ns | ~330x |

So a green channel rendezvous on this primitive costs ~30 ns of switch overhead
vs ~1800 ns for the pthread path — the switch cost stops being the throughput
ceiling. Combined with the thread-count ceiling win (6-7k pthreads -> heap-bound
fibers), this is the performance basis for the two-tier model.
