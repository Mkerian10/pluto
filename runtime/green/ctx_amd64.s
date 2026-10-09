// Register-only cooperative context switch for the green-task scheduler (#369),
// x86-64 SysV. Saves callee-saved rbx/rbp/r12-r15 to the current stack, stores
// sp into *from_sp, loads sp from to_sp, restores, returns into the resumed
// context. No sigprocmask syscall (unlike ucontext). Both symbol spellings are
// defined so the one object serves macOS (_-prefixed) and Linux.
//
//   void pluto_ctx_swap(void **from_sp, void *to_sp)   // rdi=&from_sp, rsi=to_sp
//
// NOTE: validated by assembly only on the aarch64 dev host; must be run-tested
// on x86-64 hardware/CI before the scheduler depends on it.
.text
.global _pluto_ctx_swap
.global pluto_ctx_swap
_pluto_ctx_swap:
pluto_ctx_swap:
    pushq %rbp
    pushq %rbx
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    movq  %rsp, (%rdi)
    movq  %rsi, %rsp
    popq  %r15
    popq  %r14
    popq  %r13
    popq  %r12
    popq  %rbx
    popq  %rbp
    ret
