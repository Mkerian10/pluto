// Validates the central GC-soundness claim for cooperative green tasks: a heap
// root a task holds ONLY on its stack/regs (here: the task argument, which is
// not reloadable from any global) lies within [sp, stack_top) after a park —
// stack locals above sp, plus callee-saved registers pushed into that range by
// pluto_ctx_swap. A conservative scan of that range finds it, with NO separate
// register snapshot (unlike preemptive thread parking).
#include "green_sched.h"
#include "green_chan.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

static GChan *ch;
static void * volatile observed;     // sink so the root is genuinely live post-park

static void fiber(void *arg){
    void *p = arg;             // reachable ONLY via the fiber's stack/regs
    gchan_recv(ch);            // empty -> PARKS; p must survive across it
    observed = p;              // genuine use after the park
}
static int scan_range_for(void *lo, void *hi, void *needle){
    for (char *w=(char*)lo; w + sizeof(void*) <= (char*)hi; w += sizeof(void*)) {
        void *v; __builtin_memcpy(&v, w, sizeof(void*));
        if (v == needle) return 1;
    }
    return 0;
}
int main(){
    void *sentinel = malloc(64);
    ch = gchan_new(1);
    GTask *t = green_spawn(fiber, sentinel, 64*1024);
    green_run();
    void *lo, *hi; green_task_live_range(t, &lo, &hi);
    int found = scan_range_for(lo, hi, sentinel);
    printf("parked-fiber live range = [%p, %p) (%ld bytes); arg-root %s\n",
           lo, hi, (long)((char*)hi-(char*)lo), found ? "FOUND -> scan is sound" : "NOT FOUND");
    return found ? 0 : 1;
}
