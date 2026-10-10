#include "builtins.h"
#include <stdio.h>
#include <stdint.h>
#include <string.h>
// Stubs normally provided by threading.c/builtins.c.
void __pluto_register_exit_check(void){}
long __pluto_rwlock_init(void){return 0;}
void __pluto_rwlock_destroy(long x){(void)x;}

static char fiber_stack[64*1024] __attribute__((aligned(16)));

// Alloc a sentinel, stash its ONLY reference on the fake fiber stack, register
// a green context over [sp, top). Returns the ctx handle; the sentinel pointer
// does NOT escape (so it can't linger on the main stack and mask the test).
static void *__attribute__((noinline)) setup(void){
    void *sentinel = gc_alloc(32, GC_TAG_STRING, 0);
    memset(sentinel, 0xAB, 32);
    char *top = fiber_stack + sizeof(fiber_stack);
    top = (char*)((uintptr_t)top & ~(uintptr_t)15);
    char *sp = top - 1024;                 // simulate a switched-out fiber
    *(void**)(top - 128) = sentinel;       // the only live reference
    return __pluto_gc_register_green_context(top, sp);
}

int main(void){
    int anchor; __pluto_gc_init(&anchor);
    void *ctx = setup();
    size_t garbage_peak;
    for (int i=0;i<2000;i++){ void *g = gc_alloc(64, GC_TAG_STRING, 0); (void)g; }
    garbage_peak = __pluto_gc_bytes_allocated();
    __pluto_gc_collect();
    size_t after = __pluto_gc_bytes_allocated();
    int ok = (after >= 32 && after < 4096);
    printf("garbage_peak=%zu  after_collect=%zu  -> %s\n", garbage_peak, after,
           ok ? "SENTINEL SURVIVED via green context, garbage swept (OK)"
              : (after==0 ? "SENTINEL SWEPT (FAIL)" : "UNEXPECTED"));
    (void)ctx;
    return ok ? 0 : 1;
}
