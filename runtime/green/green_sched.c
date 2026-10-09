// See green_sched.h. Single-threaded: no locks, no atomics. The register-only
// `pluto_ctx_swap` saves the current context's sp into *from and resumes the
// context at `to`.
#include "green_sched.h"
#include <stdlib.h>
#include <stdint.h>
#include <string.h>

extern void pluto_ctx_swap(void **from_sp, void *to_sp);

struct GTask {
    void *sp;              // saved stack pointer (NULL once finished)
    char *stack;           // malloc'd stack base (freed when reaped)
    void (*fn)(void *);
    void *arg;
    int done;
    GTask *next;           // ready-queue link
};

typedef struct {
    void *sched_sp;        // the scheduler loop's own saved context
    GTask *head, *tail;    // FIFO ready queue
    GTask *current;        // running task (NULL while in the scheduler)
    long created;
    long live;             // not-yet-finished tasks
} GScheduler;

// One scheduler per OS thread. Thread-local so multiple schedulers (future
// thread-per-core) never share a queue — shared-nothing by construction.
static _Thread_local GScheduler g_sched;

static void rq_push(GTask *t) {
    t->next = NULL;
    if (g_sched.tail) g_sched.tail->next = t; else g_sched.head = t;
    g_sched.tail = t;
}
static GTask *rq_pop(void) {
    GTask *t = g_sched.head;
    if (t) { g_sched.head = t->next; if (!g_sched.head) g_sched.tail = NULL; }
    return t;
}

// Prime a fresh stack so the first swap-in lands in `entry` on a clean stack.
// Mirrors the per-arch contract documented in runtime/green/README.md.
static void *prime_context(char *stack_top, void (*entry)(void)) {
    uintptr_t top = (uintptr_t)stack_top & ~(uintptr_t)15;  // 16-align
#if defined(__aarch64__)
    char *base = (char *)(top - 160);
    memset(base, 0, 160);
    *(void **)(base + 88) = (void *)entry;   // lr (x30) slot
    return base;
#elif defined(__x86_64__)
    char *base = (char *)(top - 64);
    memset(base, 0, 64);
    *(void **)(base + 48) = (void *)entry;   // return-address slot
    return base;
#else
#error "green scheduler: unsupported architecture"
#endif
}

// First-run trampoline: runs the task body, marks it finished, and returns to
// the scheduler forever (never falls off the end of the stack).
static void gtask_trampoline(void) {
    GTask *t = g_sched.current;
    t->fn(t->arg);
    t->done = 1;
    g_sched.live--;
    pluto_ctx_swap(&t->sp, g_sched.sched_sp);  // no return
}

GTask *green_spawn(void (*fn)(void *), void *arg, size_t stack_size) {
    GTask *t = (GTask *)calloc(1, sizeof(GTask));
    t->stack = (char *)malloc(stack_size);
    t->fn = fn;
    t->arg = arg;
    t->sp = prime_context(t->stack + stack_size, gtask_trampoline);
    g_sched.created++;
    g_sched.live++;
    rq_push(t);
    return t;
}

void green_yield(void) {
    GTask *t = g_sched.current;
    rq_push(t);                                 // back of the ready queue
    pluto_ctx_swap(&t->sp, g_sched.sched_sp);   // resume scheduler; come back later
}

void green_run(void) {
    for (;;) {
        GTask *t = rq_pop();
        if (!t) break;
        g_sched.current = t;
        pluto_ctx_swap(&g_sched.sched_sp, t->sp);  // run until yield/finish
        g_sched.current = NULL;
        if (t->done) { free(t->stack); free(t); }  // reap finished task
    }
}

long green_task_count(void) { return g_sched.created; }

void green_park(void) {
    GTask *t = g_sched.current;
    // NOT re-queued: the waker must green_wake(t) to make it runnable again.
    pluto_ctx_swap(&t->sp, g_sched.sched_sp);
}
void green_wake(GTask *t) { rq_push(t); }
GTask *green_current(void) { return g_sched.current; }
long green_live(void) { return g_sched.live; }
