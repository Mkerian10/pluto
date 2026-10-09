// Register-only cooperative context switch for the green-task scheduler (#369).
// Saves the AAPCS64 callee-saved set (x19-x28, fp/lr, d8-d15) to the current
// stack, stores sp into *from_sp, loads sp from to_sp, restores, and returns
// into the resumed context. ~15 ns/switch vs ~440 ns for ucontext swapcontext
// (no sigprocmask syscall). No heap, no syscalls.
//
//   void pluto_ctx_swap(void **from_sp, void *to_sp)   // x0=&from_sp, x1=to_sp
.text
.global _pluto_ctx_swap
.global pluto_ctx_swap
.p2align 2
_pluto_ctx_swap:
pluto_ctx_swap:
    sub  sp, sp, #160
    stp  x19, x20, [sp, #0]
    stp  x21, x22, [sp, #16]
    stp  x23, x24, [sp, #32]
    stp  x25, x26, [sp, #48]
    stp  x27, x28, [sp, #64]
    stp  x29, x30, [sp, #80]
    stp  d8,  d9,  [sp, #96]
    stp  d10, d11, [sp, #112]
    stp  d12, d13, [sp, #128]
    stp  d14, d15, [sp, #144]
    mov  x2, sp
    str  x2, [x0]
    mov  sp, x1
    ldp  x19, x20, [sp, #0]
    ldp  x21, x22, [sp, #16]
    ldp  x23, x24, [sp, #32]
    ldp  x25, x26, [sp, #48]
    ldp  x27, x28, [sp, #64]
    ldp  x29, x30, [sp, #80]
    ldp  d8,  d9,  [sp, #96]
    ldp  d10, d11, [sp, #112]
    ldp  d12, d13, [sp, #128]
    ldp  d14, d15, [sp, #144]
    add  sp, sp, #160
    ret
