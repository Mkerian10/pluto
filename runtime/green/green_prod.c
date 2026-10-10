// Production green scheduler skeleton (#369 integration checkpoint 1): a single
// scheduler pthread with a thread-safe ready queue, lazy startup, and
// cross-thread result delivery. Runs tasks to completion over the register-only
// context switch. GC-context registration, real Pluto closures, cooperative
// park/wake and codegen wiring come in later checkpoints; this proves the
// cross-thread spawn/await skeleton in isolation.
//
// Production only — uses pthreads. Under PLUTO_TEST_MODE green routes to the
// DPOR fiber scheduler instead (see rfc-green-tasks.md), so this file is inert
// there.
#ifndef PLUTO_TEST_MODE
#include <pthread.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>

extern void pluto_ctx_swap(void **from_sp, void *to_sp);

// GC coordination (marksweep.c in the real build; stubbed in standalone tests).
extern void *__pluto_gc_register_green_context(void *stack_top, void *live_sp);
extern void __pluto_gc_unregister_green_context(void *handle);
extern void __pluto_gc_register_thread_stack(void *lo, void *hi);
extern void __pluto_gc_enter_safe_region(void);
extern void __pluto_gc_leave_safe_region(void);

typedef struct GProdTask {
    long (*fn)(void *);     // stand-in for a Pluto closure call
    void *arg;
    long result;
    int done;
    pthread_mutex_t mu;
    pthread_cond_t cv;
    struct GProdTask *next; // ready-queue link
    char *stack;
    size_t stack_size;
    void *sp;               // saved fiber sp
} GProdTask;

static pthread_mutex_t gq_mu = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t  gq_wake = PTHREAD_COND_INITIALIZER;
static GProdTask *gq_head, *gq_tail;
static pthread_t gq_thread;
static int gq_started = 0;

// The scheduler thread's own context + the task it is running (one scheduler,
// so a single pair suffices; thread-local would generalize to N schedulers).
static void *gsched_sp;
static GProdTask *gsched_current;

static void *prime_ctx(char *stack_top, void (*entry)(void)) {
    uintptr_t top = (uintptr_t)stack_top & ~(uintptr_t)15;
#if defined(__aarch64__)
    char *base = (char *)(top - 160); memset(base, 0, 160);
    *(void **)(base + 88) = (void *)entry; return base;
#elif defined(__x86_64__)
    char *base = (char *)(top - 64); memset(base, 0, 64);
    *(void **)(base + 48) = (void *)entry; return base;
#else
#error "unsupported arch"
#endif
}

static void gprod_trampoline(void) {
    GProdTask *t = gsched_current;
    // Running fiber: register its context with live_sp = stack base, so a
    // collection mid-run conservatively scans the WHOLE fiber stack and never
    // misses a live frame (cp2 running-fiber resolution).
    void *gc_ctx = __pluto_gc_register_green_context(t->stack + t->stack_size, t->stack);
    long r = t->fn(t->arg);
    __pluto_gc_unregister_green_context(gc_ctx);
    pthread_mutex_lock(&t->mu);
    t->result = r;
    t->done = 1;
    pthread_cond_signal(&t->cv);
    pthread_mutex_unlock(&t->mu);
    pluto_ctx_swap(&t->sp, gsched_sp);   // back to scheduler, no return
}

static void *gprod_scheduler(void *_unused) {
    (void)_unused;
    // Register the scheduler thread's own C stack so the collector scans the
    // scheduler loop's frames (and any suspended-at-swap state) as roots.
    {
        pthread_t self = pthread_self();
#ifdef __APPLE__
        void *hi = pthread_get_stackaddr_np(self);
        void *lo = (char *)hi - pthread_get_stacksize_np(self);
#else
        void *lo = NULL, *hi = NULL; size_t sz = 0;
        pthread_attr_t a; pthread_getattr_np(self, &a);
        pthread_attr_getstack(&a, &lo, &sz); hi = (char *)lo + sz; pthread_attr_destroy(&a);
#endif
        __pluto_gc_register_thread_stack(lo, hi);
    }
    for (;;) {
        pthread_mutex_lock(&gq_mu);
        // Idle wait counts as a GC-safe region so stop-the-world converges
        // while the scheduler sleeps (it holds no heap, like a channel wait).
        while (!gq_head) {
            __pluto_gc_enter_safe_region();
            pthread_cond_wait(&gq_wake, &gq_mu);
            __pluto_gc_leave_safe_region();
        }
        GProdTask *t = gq_head;
        gq_head = t->next;
        if (!gq_head) gq_tail = NULL;
        pthread_mutex_unlock(&gq_mu);

        gsched_current = t;
        pluto_ctx_swap(&gsched_sp, t->sp);   // run until it finishes
        gsched_current = NULL;
        free(t->stack);   // task reaped by its awaiter via the handle; stack is ours
    }
    return NULL;
}

static void gprod_ensure_started(void) {
    pthread_mutex_lock(&gq_mu);
    if (!gq_started) {
        gq_started = 1;
        pthread_create(&gq_thread, NULL, gprod_scheduler, NULL);
    }
    pthread_mutex_unlock(&gq_mu);
}

GProdTask *__pluto_green_prod_spawn(long (*fn)(void *), void *arg, size_t stack_size) {
    gprod_ensure_started();
    GProdTask *t = (GProdTask *)calloc(1, sizeof(GProdTask));
    t->fn = fn; t->arg = arg;
    t->stack = (char *)malloc(stack_size);
    t->stack_size = stack_size;
    t->sp = prime_ctx(t->stack + stack_size, gprod_trampoline);
    pthread_mutex_init(&t->mu, NULL);
    pthread_cond_init(&t->cv, NULL);
    pthread_mutex_lock(&gq_mu);
    t->next = NULL;
    if (gq_tail) gq_tail->next = t; else gq_head = t;
    gq_tail = t;
    pthread_cond_signal(&gq_wake);
    pthread_mutex_unlock(&gq_mu);
    return t;
}

long __pluto_green_prod_get(GProdTask *t) {
    pthread_mutex_lock(&t->mu);
    while (!t->done) pthread_cond_wait(&t->cv, &t->mu);
    long r = t->result;
    pthread_mutex_unlock(&t->mu);
    return r;
}
#endif
