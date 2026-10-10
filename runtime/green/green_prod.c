// Production green scheduler (#369). A single scheduler pthread with a
// thread-safe ready queue, lazy startup, and cross-thread result delivery,
// running fibers over the register-only context switch.
//
// cp3b: run-to-completion submit path (real Pluto closures via a callback).
// cp4: cooperative PARK / YIELD / WAKE so a fiber that awaits a task or blocks
// on a channel suspends the FIBER (yielding the scheduler to its peers) instead
// of blocking the whole scheduler thread. Wakes are thread-safe: a pthread or
// the main thread completing an awaited task re-readies the parked fiber.
//
// Race-freedom (the gopark pattern): a parking fiber records NOTHING about
// itself before it switches out. It stashes a handoff fn + arg, sets state
// PARK, and swaps to the scheduler. The scheduler — with the fiber fully
// switched out — runs the handoff on its OWN stack. Only the handoff publishes
// the fiber as a waiter (under the awaited object's lock). So a wake can never
// reach a fiber that has not finished switching out, which would otherwise run
// one fiber on two contexts.
//
// Production only — uses pthreads. Under PLUTO_TEST_MODE green routes to the
// DPOR fiber scheduler instead (see rfc-green-tasks.md), so this file is inert
// there.
#ifndef PLUTO_TEST_MODE
#include <pthread.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
#include <time.h>

extern void pluto_ctx_swap(void **from_sp, void *to_sp);

// GC coordination (marksweep.c in the real build; stubbed in standalone tests).
extern void *__pluto_gc_register_green_context(void *stack_top, void *live_sp);
extern void __pluto_gc_green_set_live_sp(void *handle, void *live_sp);
extern void __pluto_gc_unregister_green_context(void *handle);
extern void __pluto_gc_register_thread_stack(void *lo, void *hi);
extern void __pluto_gc_enter_safe_region(void);
extern void __pluto_gc_leave_safe_region(void);

// Fiber lifecycle state, read by the scheduler after each swap-back.
enum { GRUN = 0, GDONE, GYIELD, GPARK };

typedef struct GProdTask {
    long (*fn)(void *);     // cp1 self-contained path (result via result/cv below)
    void *arg;
    long result;
    int done;
    pthread_mutex_t mu;
    pthread_cond_t cv;
    struct GProdTask *next; // ready-queue link
    char *stack;            // usable stack region [stack, stack+stack_size)
    size_t stack_size;
    void *stack_map;        // mmap base (one guard page below `stack`)
    size_t stack_map_size;  // guard + stack_size, for munmap
    void *sp;               // saved fiber sp
    int state;              // GRUN / GDONE / GYIELD / GPARK (cp4)
    void (*park_fn)(void *);// cp4: handoff run on the scheduler stack after the
    void *park_arg;         //      fiber parks (publishes it as a waiter)
    void *gc_ctx;           // GC green-context handle (held across parks)
    long deadline_ns;       // cp6: CLOCK_REALTIME wake deadline when timer-parked
    struct GProdTask *timer_next; // cp6: link in the sorted timer list
    // Submit path (__pluto_green_submit): when submit_run != NULL the fiber
    // calls submit_run(submit_job) and the callback owns result delivery (e.g.
    // into a Pluto Task<T>); the scheduler then frees the whole GProdTask.
    void (*submit_run)(void *);
    void *submit_job;
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

// cp6 timer wheel: fibers that sleep (and, later, time out on a channel) park
// here instead of blocking the scheduler. Sorted ascending by deadline, guarded
// by gq_mu. (A sorted list is O(n) insert — fine for modest timer counts; a
// heap is the later optimization for many concurrent sleepers.)
static GProdTask *timer_head;

static long gprod_now_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    return (long)ts.tv_sec * 1000000000L + (long)ts.tv_nsec;
}

// Insert t into the sorted timer list. Caller holds gq_mu.
static void timer_insert(GProdTask *t) {
    GProdTask **pp = &timer_head;
    while (*pp && (*pp)->deadline_ns <= t->deadline_ns) pp = &(*pp)->timer_next;
    t->timer_next = *pp;
    *pp = t;
}

// Move every timer whose deadline has passed onto the ready queue. Caller holds
// gq_mu (so the inline enqueue is safe without gq_push's own lock).
static void timer_fire_expired(long now) {
    while (timer_head && timer_head->deadline_ns <= now) {
        GProdTask *t = timer_head;
        timer_head = t->timer_next;
        t->timer_next = NULL;
        t->state = GRUN;
        t->next = NULL;
        if (gq_tail) gq_tail->next = t; else gq_head = t;
        gq_tail = t;
    }
}
// Set on the scheduler thread only, so __pluto_green_self() can tell a fiber
// (running on the scheduler thread) from an ordinary pthread / the main thread.
static __thread int gt_on_sched = 0;

// Enqueue onto the ready queue. Caller must NOT hold gq_mu.
static void gq_push(GProdTask *t) {
    pthread_mutex_lock(&gq_mu);
    t->next = NULL;
    if (gq_tail) gq_tail->next = t; else gq_head = t;
    gq_tail = t;
    pthread_cond_signal(&gq_wake);
    pthread_mutex_unlock(&gq_mu);
}

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

// ── Cooperative primitives (called from fiber context) ───────────────────────

// The currently-running fiber, or NULL if the caller is not a scheduler fiber
// (an ordinary pthread or the main thread). Lets await/channel ops choose
// between parking (fiber) and a blocking cond wait (pthread/main).
void *__pluto_green_self(void) {
    return gt_on_sched ? (void *)gsched_current : NULL;
}

// Yield the scheduler to ready peers, then resume. No-op cost beyond two
// register-only switches when the fiber is the only one ready.
void __pluto_green_yield(void) {
    GProdTask *t = gsched_current;
    if (!t) return;
    t->state = GYIELD;
    pluto_ctx_swap(&t->sp, gsched_sp);
}

// Suspend the current fiber. After it has fully switched out, the scheduler
// runs handoff(arg) on its own stack; handoff publishes this fiber as a waiter
// on whatever it is blocked on (under that object's lock), or re-readies it at
// once if the event already happened. The fiber resumes here once woken.
void __pluto_green_park(void (*handoff)(void *), void *arg) {
    GProdTask *t = gsched_current;
    t->park_fn = handoff;
    t->park_arg = arg;
    t->state = GPARK;
    pluto_ctx_swap(&t->sp, gsched_sp);
}

// cp6 sleep handoff: on the scheduler stack with the fiber switched out, insert
// it into the timer list (under gq_mu). The scheduler fires it at its deadline.
static void sleep_handoff(void *arg) {
    GProdTask *t = (GProdTask *)arg;
    pthread_mutex_lock(&gq_mu);
    timer_insert(t);
    pthread_mutex_unlock(&gq_mu);
}

// Cooperative sleep: park the current fiber until `ns` from now, yielding the
// scheduler to its peers (instead of a nanosleep that would stall them all).
// Caller must be a fiber (the runtime routes non-fiber sleep to nanosleep).
void __pluto_green_sleep_ns(long ns) {
    GProdTask *t = gsched_current;
    if (!t) return;
    t->deadline_ns = gprod_now_ns() + (ns > 0 ? ns : 0);
    __pluto_green_park(sleep_handoff, t);   // resumes once the deadline fires
}

// Re-ready a parked fiber. Thread-safe — called from the completer's thread
// (a pthread finishing a spawn task, a peer fiber, or the main thread). The
// handoff that published `fiber` as a waiter ran before any wake could see it,
// so `fiber` is guaranteed fully switched out here.
void __pluto_green_wake(void *fiber) {
    GProdTask *t = (GProdTask *)fiber;
    t->state = GRUN;
    gq_push(t);
}

// ── Scheduler ────────────────────────────────────────────────────────────────

static void gprod_trampoline(void) {
    GProdTask *t = gsched_current;
    // Running fiber: register its context with live_sp = stack base, so a
    // collection mid-run conservatively scans the WHOLE fiber stack and never
    // misses a live frame (cp2 running-fiber resolution). Held across parks so
    // a suspended fiber's stack stays scannable; torn down only at finish.
    t->gc_ctx = __pluto_gc_register_green_context(t->stack + t->stack_size, t->stack);
    if (t->submit_run) {
        t->submit_run(t->submit_job);
        __pluto_gc_unregister_green_context(t->gc_ctx);
        t->state = GDONE;
        pluto_ctx_swap(&t->sp, gsched_sp);   // back to scheduler, no return
    }
    long r = t->fn(t->arg);
    __pluto_gc_unregister_green_context(t->gc_ctx);
    pthread_mutex_lock(&t->mu);
    t->result = r;
    t->done = 1;
    pthread_cond_signal(&t->cv);
    pthread_mutex_unlock(&t->mu);
    t->state = GDONE;
    pluto_ctx_swap(&t->sp, gsched_sp);   // back to scheduler, no return
}

static void *gprod_scheduler(void *_unused) {
    (void)_unused;
    gt_on_sched = 1;
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
        timer_fire_expired(gprod_now_ns());
        // Idle wait counts as a GC-safe region so stop-the-world converges
        // while the scheduler sleeps (it holds no heap, like a channel wait).
        // With timers pending, wait only until the nearest deadline, then fire.
        while (!gq_head) {
            __pluto_gc_enter_safe_region();
            if (timer_head) {
                long d = timer_head->deadline_ns;
                struct timespec ts = { (time_t)(d / 1000000000L), (long)(d % 1000000000L) };
                pthread_cond_timedwait(&gq_wake, &gq_mu, &ts);
            } else {
                pthread_cond_wait(&gq_wake, &gq_mu);
            }
            __pluto_gc_leave_safe_region();
            timer_fire_expired(gprod_now_ns());
        }
        GProdTask *t = gq_head;
        gq_head = t->next;
        if (!gq_head) gq_tail = NULL;
        pthread_mutex_unlock(&gq_mu);

        gsched_current = t;
        int is_submit = (t->submit_run != NULL);
        // About to run: the fiber's sp will move, so scan its whole stack
        // conservatively (live_sp = base) for any collection while it runs.
        // (On first run gc_ctx is NULL until the trampoline registers it, also
        // at base.) A plain store — see __pluto_gc_green_set_live_sp.
        if (t->gc_ctx) __pluto_gc_green_set_live_sp(t->gc_ctx, t->stack);
        pluto_ctx_swap(&gsched_sp, t->sp);   // run until it yields/parks/finishes
        gsched_current = NULL;
        // Switched back out: unless finished, the fiber is suspended at t->sp,
        // so only [t->sp, stack_top) is live — scan precisely. Without this a
        // parked fiber would be scanned [base, top) every collection, faulting
        // in its whole 512 KB stack (idle fibers must stay cheap).
        if (t->gc_ctx && t->state != GDONE) __pluto_gc_green_set_live_sp(t->gc_ctx, t->sp);

        switch (t->state) {
        case GYIELD:
            t->state = GRUN;
            gq_push(t);                      // round-robin: back of the queue
            break;
        case GPARK: {
            // Fiber fully switched out: run its handoff on the scheduler stack.
            // The handoff either re-readies t (event already happened) or
            // records t as a waiter to be woken later. Until then t is off all
            // queues — only a future __pluto_green_wake re-enqueues it.
            void (*pf)(void *) = t->park_fn; void *pa = t->park_arg;
            t->park_fn = NULL; t->park_arg = NULL;
            if (pf) pf(pa);
            break;
        }
        case GDONE:
        default:
            munmap(t->stack_map, t->stack_map_size);
            // Submit tasks have no awaiter on the GProdTask (delivery went
            // through the callback), so the scheduler owns and frees the
            // handle too. cp1 tasks are read via __pluto_green_prod_get, so
            // only their stack is freed here.
            if (is_submit) free(t);
            break;
        }
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

static GProdTask *gprod_alloc(size_t stack_size) {
    GProdTask *t = (GProdTask *)calloc(1, sizeof(GProdTask));
    // Fiber stacks grow DOWN, so a one-page PROT_NONE guard at the LOW end
    // turns a stack overflow (deep green recursion) into an immediate SIGSEGV
    // instead of silent corruption of whatever the old malloc'd block abutted.
    long pg = sysconf(_SC_PAGESIZE);
    size_t guard = (pg > 0) ? (size_t)pg : 16384;
    size_t total = guard + stack_size;
    char *map = (char *)mmap(NULL, total, PROT_READ | PROT_WRITE,
                             MAP_PRIVATE | MAP_ANON, -1, 0);
    if (map == MAP_FAILED) { abort(); }
    mprotect(map, guard, PROT_NONE);          // low guard page
    t->stack_map = map;
    t->stack_map_size = total;
    t->stack = map + guard;                   // usable region above the guard
    t->stack_size = stack_size;
    t->sp = prime_ctx(t->stack + stack_size, gprod_trampoline);  // top (high)
    t->state = GRUN;
    return t;
}

GProdTask *__pluto_green_prod_spawn(long (*fn)(void *), void *arg, size_t stack_size) {
    gprod_ensure_started();
    GProdTask *t = gprod_alloc(stack_size);
    t->fn = fn; t->arg = arg;
    pthread_mutex_init(&t->mu, NULL);
    pthread_cond_init(&t->cv, NULL);
    gq_push(t);
    return t;
}

long __pluto_green_prod_get(GProdTask *t) {
    pthread_mutex_lock(&t->mu);
    while (!t->done) pthread_cond_wait(&t->cv, &t->mu);
    long r = t->result;
    pthread_mutex_unlock(&t->mu);
    return r;
}

// Submit a green task that runs `run(job)` to completion on the scheduler, with
// its fiber GC-context registered. Result delivery is the callback's job (e.g.
// into a Pluto Task<T> via its TaskSync). The scheduler owns and frees the
// fiber. This is the entry point threading.c's __pluto_green_spawn uses.
void __pluto_green_submit(void (*run)(void *), void *job, size_t stack_size) {
    gprod_ensure_started();
    GProdTask *t = gprod_alloc(stack_size);
    t->submit_run = run;
    t->submit_job = job;
    gq_push(t);
}
#endif
