#define _XOPEN_SOURCE 700
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <time.h>
#include <ucontext.h>
static uint64_t ns(){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (uint64_t)t.tv_sec*1000000000ull+t.tv_nsec;}
extern void pluto_ctx_swap(void **from_sp, void *to_sp);
static void *main_sp, *fiber_sp;
static volatile long counter;
static char fstack[256*1024] __attribute__((aligned(16)));
static void fiber_entry(void){ for(;;){ counter++; pluto_ctx_swap(&fiber_sp, main_sp); } }
static void prime(void){
    char *top=fstack+sizeof(fstack);
    top=(char*)((uintptr_t)top & ~(uintptr_t)15);
    char *base=top-160; memset(base,0,160);
    *(void**)(base+88)=(void*)fiber_entry;  // x30/lr slot
    fiber_sp=base;
}
int main(){
    prime();
    long N=2000000;
    // correctness: first switch should run fiber_entry once (counter→1) then return
    pluto_ctx_swap(&main_sp, fiber_sp);
    if(counter!=1){ printf("CORRECTNESS FAIL counter=%ld\n",counter); return 1; }
    uint64_t t0=ns();
    for(long i=0;i<N;i++) pluto_ctx_swap(&main_sp, fiber_sp);
    uint64_t t1=ns();
    printf("asm ctx swap round-trip: %.1f ns  (counter=%ld, expect %ld)\n",(double)(t1-t0)/N, counter, N+1);
    return 0;
}
