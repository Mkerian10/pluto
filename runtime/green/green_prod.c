// Production green scheduler (#369). N scheduler pthreads (thread-per-core by
// default), each with its own ready queue + timer wheel, running fibers over
// the register-only context switch.
//
// cp3b: run-to-completion submit path (real Pluto closures via a callback).
// cp4: cooperative PARK / YIELD / WAKE so a fiber that awaits a task or blocks
// on a channel suspends the FIBER (yielding the scheduler to its peers) instead
// of blocking the whole scheduler thread.
// cp6: timer wheel (cooperative sleep / timed waits).
// MULTI-SCHEDULER (structural): a green task is assigned to one scheduler at
// submit (round-robin) and runs there for its whole life — SHARED-NOTHING, no
// work-stealing, no migration. Because a fiber never changes threads, every
// single-scheduler invariant still holds: thread-local runtime state
// (__pluto_current_error/_task) stays consistent, a mutex is locked and
// unlocked on one thread, and each fiber's GC live_sp has exactly one writer
// (its home scheduler). Cross-scheduler wakes (a completer on another thread
// re-readying a parked fiber) enqueue onto the fiber's HOME scheduler.
//
// Race-freedom (the gopark pattern): a parking fiber records NOTHING about
// itself before it switches out. It stashes a handoff fn + arg, sets state
// PARK, and swaps to the scheduler, which runs the handoff on its OWN stack —
// only then is the fiber published as a waiter, so a wake can never reach a
// fiber still mid-switch.
//
// Production only — uses pthreads. Under PLUTO_TEST_MODE green routes to the
// DPOR fiber scheduler instead, so this file is inert there.
#ifndef PLUTO_TEST_MODE
#include <pthread.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <stdio.h>
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

// Per-task runtime state that lives in THREAD-LOCAL storage (set by green_run_cb
// and read by the channel ops' cancellation check). A fiber that migrates
// between scheduler threads must carry these with it, or the origin thread is
// left pointing at a freed task. The scheduler saves/restores them around every
// fiber run (mirrors the test-mode fiber scheduler's saved_current_task).
extern __thread void *__pluto_current_error;
extern __thread void *__pluto_current_error_type;
extern __thread long *__pluto_current_task;

// Fiber lifecycle state, read by the scheduler after each swap-back.
enum { GRUN = 0, GDONE, GYIELD, GPARK };

typedef struct GProdSched GProdSched;

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
    void (*park_fn)(void *);// cp4: handoff run on the scheduler stack after park
    void *park_arg;
    void *gc_ctx;           // GC green-context handle (held across parks)
    long deadline_ns;       // cp6: CLOCK_REALTIME wake deadline when timer-parked
    struct GProdTask *timer_next; // cp6: link in the sorted timer list
    GProdSched *home;       // scheduler this task currently lives on
    // Per-fiber copy of the thread-local runtime state, so it follows the fiber
    // across scheduler threads (migration safety).
    long *sv_task;
    void *sv_error;
    void *sv_error_type;
    // Submit path (__pluto_green_submit): when submit_run != NULL the fiber
    // calls submit_run(submit_job) and the callback owns result delivery; the
    // scheduler then frees the whole GProdTask.
    void (*submit_run)(void *);
    void *submit_job;
} GProdTask;

// One scheduler = one OS thread + its own ready queue, timer wheel, and running
// context. Everything here is touched only by this scheduler's own thread
// except the ready queue + its mutex/cond, which cross-scheduler/other-thread
// wakes push onto under `mu`.
struct GProdSched {
    pthread_mutex_t mu;
    pthread_cond_t  wake;
    GProdTask *head, *tail;     // REMOTE ready queue: cross-thread wakes push
                                //   here under `mu`; the scheduler drains it.
    GProdTask *local_head, *local_tail; // LOCAL ready queue: touched ONLY by
                                //   this scheduler's own thread (yields, and
                                //   same-thread wakes), so NO lock — the hot
                                //   path for channel handoffs / await on one
                                //   scheduler avoids the mutex entirely.
    GProdTask *timer_head;      // sorted-by-deadline timer list (guarded by mu)
    void *sched_sp;             // the scheduler loop's own saved context
    GProdTask *current;         // the fiber it is currently running
    pthread_t thread;
};

#define MAX_SCHED 64
static GProdSched g_scheds[MAX_SCHED];
static int g_nsched = 0;                 // number of live schedulers
static int g_started = 0;
static pthread_mutex_t g_start_mu = PTHREAD_MUTEX_INITIALIZER;
static unsigned long g_rr = 0;           // round-robin assignment counter
// EXPERIMENTAL (default off): wake-locality / fiber migration. When set, a
// woken fiber moves to the waker's scheduler so communicating partners
// converge. INCOMPLETE: the first blocker (a SEGV from stale thread-local
// current_task after migration) is fixed by the per-fiber TLS save/restore in
// the scheduler loop, but a second channel-migration issue remains (the Pluto
// ping-pong exits with an uncaught error), so this stays off. See
// __pluto_green_wake and rfc-green-tasks.md.
static int g_wake_local = 0;
// DEBUG (default off): assert a task's saved sp lies within its own stack
// before swapping into it — catches a corrupt/migrated context at the swap
// instead of as a jump to a garbage address.
static int g_sp_check = 0;

// The scheduler this thread IS, or NULL on a non-scheduler thread (main/spawn).
// Replaces the old gt_on_sched flag and names the running scheduler for the
// cooperative primitives.
static __thread GProdSched *cur_sched = NULL;

static long gprod_now_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    return (long)ts.tv_sec * 1000000000L + (long)ts.tv_nsec;
}

// Insert t into s's sorted timer list. Caller holds s->mu.
static void timer_insert(GProdSched *s, GProdTask *t) {
    GProdTask **pp = &s->timer_head;
    while (*pp && (*pp)->deadline_ns <= t->deadline_ns) pp = &(*pp)->timer_next;
    t->timer_next = *pp;
    *pp = t;
}

// Move every expired timer onto s's ready queue. Caller holds s->mu.
static void timer_fire_expired(GProdSched *s, long now) {
    while (s->timer_head && s->timer_head->deadline_ns <= now) {
        GProdTask *t = s->timer_head;
        s->timer_head = t->timer_next;
        t->timer_next = NULL;
        t->state = GRUN;
        t->next = NULL;
        if (s->tail) s->tail->next = t; else s->head = t;
        s->tail = t;
    }
}

// Enqueue onto scheduler s's REMOTE ready queue under its lock + wake it. Used
// for cross-thread wakes (a different scheduler or a non-scheduler thread
// re-readying a fiber whose home is s).
static void sched_push_remote(GProdSched *s, GProdTask *t) {
    pthread_mutex_lock(&s->mu);
    t->next = NULL;
    if (s->tail) s->tail->next = t; else s->head = t;
    s->tail = t;
    pthread_cond_signal(&s->wake);
    pthread_mutex_unlock(&s->mu);
}

// Enqueue onto s's LOCAL ready queue with NO lock. ONLY valid on s's own
// scheduler thread (a yield, or a wake whose waker runs on s's thread).
static void sched_push_local(GProdSched *s, GProdTask *t) {
    t->next = NULL;
    if (s->local_tail) s->local_tail->next = t; else s->local_head = t;
    s->local_tail = t;
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

// The currently-running fiber, or NULL if the caller is not a scheduler fiber.
void *__pluto_green_self(void) {
    return cur_sched ? (void *)cur_sched->current : NULL;
}

// Yield to ready peers on this scheduler, then resume.
void __pluto_green_yield(void) {
    GProdSched *s = cur_sched;
    if (!s || !s->current) return;
    s->current->state = GYIELD;
    pluto_ctx_swap(&s->current->sp, s->sched_sp);
}

// Suspend the current fiber; the scheduler runs handoff(arg) on its own stack
// once the fiber has switched out (publishes it as a waiter, or re-readies it).
void __pluto_green_park(void (*handoff)(void *), void *arg) {
    GProdSched *s = cur_sched;
    GProdTask *t = s->current;
    t->park_fn = handoff;
    t->park_arg = arg;
    t->state = GPARK;
    pluto_ctx_swap(&t->sp, s->sched_sp);
}

// cp6 sleep handoff: on the scheduler stack with the fiber switched out, insert
// it into its home scheduler's timer list.
static void sleep_handoff(void *arg) {
    GProdTask *t = (GProdTask *)arg;
    GProdSched *s = t->home;
    pthread_mutex_lock(&s->mu);
    timer_insert(s, t);
    pthread_mutex_unlock(&s->mu);
}

// Cooperative sleep: park the current fiber until `ns` from now.
void __pluto_green_sleep_ns(long ns) {
    GProdSched *s = cur_sched;
    if (!s || !s->current) return;
    s->current->deadline_ns = gprod_now_ns() + (ns > 0 ? ns : 0);
    __pluto_green_park(sleep_handoff, s->current);
}

// Re-ready a parked fiber. Thread-safe and cross-scheduler: the fiber is
// enqueued onto its HOME scheduler — where it ran before parking and will run
// again — never the waker's. A fiber is PINNED to its home for life (no
// migration): that is what keeps thread-local runtime state, pthread-mutex
// ownership, and the single-writer GC live_sp invariant valid.
//
// Wake-locality (migrate the woken fiber to the waker's scheduler so
// communicating partners converge) is available behind PLUTO_GREEN_WAKE_LOCALITY
// but OFF by default and INCOMPLETE. Investigation so far: the first crash was a
// stale thread-local current_task left on the origin thread after migration —
// fixed by the per-fiber TLS save/restore around each run in the scheduler
// loop. A second channel-migration bug remains (the ping-pong exits with an
// uncaught error), so for now a fiber stays on its home and co-location is
// achieved statically via parent-locality placement at spawn. Finishing
// wake-locality (or a full work-stealing design) is the path to making
// multi-scheduler the default; see rfc-green-tasks.md.
void __pluto_green_wake(void *fiber) {
    GProdTask *t = (GProdTask *)fiber;
    t->state = GRUN;
    GProdSched *w = cur_sched;
    if (g_wake_local && w) { t->home = w; }  // experimental migration (off)
    // Fast path: the waker runs on the fiber's home scheduler thread, so the
    // local (lockless) queue is safe. Otherwise it's a cross-thread wake.
    if (w == t->home) sched_push_local(t->home, t);
    else              sched_push_remote(t->home, t);
}

// ── Scheduler ────────────────────────────────────────────────────────────────

static void gprod_trampoline(void) {
    GProdSched *s = cur_sched;
    GProdTask *t = s->current;
    // Running fiber: register its context with live_sp = stack base so a
    // collection mid-run conservatively scans the whole stack. Held across
    // parks; torn down at finish.
    t->gc_ctx = __pluto_gc_register_green_context(t->stack + t->stack_size, t->stack);
    if (t->submit_run) {
        t->submit_run(t->submit_job);
        __pluto_gc_unregister_green_context(t->gc_ctx);
        t->state = GDONE;
        pluto_ctx_swap(&t->sp, s->sched_sp);   // back to scheduler, no return
    }
    long r = t->fn(t->arg);
    __pluto_gc_unregister_green_context(t->gc_ctx);
    pthread_mutex_lock(&t->mu);
    t->result = r;
    t->done = 1;
    pthread_cond_signal(&t->cv);
    pthread_mutex_unlock(&t->mu);
    t->state = GDONE;
    pluto_ctx_swap(&t->sp, s->sched_sp);   // back to scheduler, no return
}

static void *gprod_scheduler(void *arg) {
    GProdSched *s = (GProdSched *)arg;
    cur_sched = s;
    // Register this scheduler thread's own C stack so the collector scans the
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
        // Fast path: run the local queue (no lock) until it drains. Only when
        // it is empty do we take the lock to pull in remote wakes / fire timers
        // / idle-wait. For a workload whose fibers hand off among themselves on
        // one scheduler (channels, await, yield), the mutex is never touched.
        if (!s->local_head) {
            pthread_mutex_lock(&s->mu);
            timer_fire_expired(s, gprod_now_ns());   // fires into the remote queue
            // Idle wait is a GC-safe region so stop-the-world converges while
            // the scheduler sleeps. With timers pending, wait to the nearest.
            while (!s->head) {
                __pluto_gc_enter_safe_region();
                if (s->timer_head) {
                    long d = s->timer_head->deadline_ns;
                    struct timespec ts = { (time_t)(d / 1000000000L), (long)(d % 1000000000L) };
                    pthread_cond_timedwait(&s->wake, &s->mu, &ts);
                } else {
                    pthread_cond_wait(&s->wake, &s->mu);
                }
                __pluto_gc_leave_safe_region();
                timer_fire_expired(s, gprod_now_ns());
            }
            // Move the whole remote chain into the local queue in one shot.
            s->local_head = s->head; s->local_tail = s->tail;
            s->head = s->tail = NULL;
            pthread_mutex_unlock(&s->mu);
        }
        GProdTask *t = s->local_head;
        s->local_head = t->next;
        if (!s->local_head) s->local_tail = NULL;

        s->current = t;
        int is_submit = (t->submit_run != NULL);
        if (g_sp_check && ((char *)t->sp < t->stack || (char *)t->sp > t->stack + t->stack_size)) {
            fprintf(stderr, "green: BAD sp %p not in [%p,%p) task=%p state=%d gc_ctx=%p\n",
                    t->sp, (void *)t->stack, (void *)(t->stack + t->stack_size), (void *)t, t->state, t->gc_ctx);
            abort();
        }
        // Running: sp moves, so scan the whole stack conservatively (plain
        // store — single writer, read only during STW).
        if (t->gc_ctx) __pluto_gc_green_set_live_sp(t->gc_ctx, t->stack);
        // Load this fiber's thread-local runtime state onto the current thread,
        // so cancellation checks etc. see its task even after a migration.
        __pluto_current_task = t->sv_task;
        __pluto_current_error = t->sv_error;
        __pluto_current_error_type = t->sv_error_type;
        pluto_ctx_swap(&s->sched_sp, t->sp);   // run until yield/park/finish
        s->current = NULL;
        // Save the fiber's thread-local state back onto the fiber and clear it
        // from this thread, so nothing leaks to the next fiber or is left stale
        // for another thread after the fiber migrates away.
        t->sv_task = __pluto_current_task;
        t->sv_error = __pluto_current_error;
        t->sv_error_type = __pluto_current_error_type;
        __pluto_current_task = NULL;
        __pluto_current_error = NULL;
        __pluto_current_error_type = NULL;
        // Suspended: only [t->sp, top) is live — scan precisely so idle fibers
        // do not fault in their whole stack.
        if (t->gc_ctx && t->state != GDONE) __pluto_gc_green_set_live_sp(t->gc_ctx, t->sp);

        switch (t->state) {
        case GYIELD:
            t->state = GRUN;
            sched_push_local(s, t);          // same thread → lockless local queue
            break;
        case GPARK: {
            void (*pf)(void *) = t->park_fn; void *pa = t->park_arg;
            t->park_fn = NULL; t->park_arg = NULL;
            if (pf) pf(pa);
            break;
        }
        case GDONE:
        default:
            munmap(t->stack_map, t->stack_map_size);
            if (is_submit) free(t);
            break;
        }
    }
    return NULL;
}

// Number of schedulers to run. DEFAULT 1 (single scheduler) — deliberately:
// with >1 scheduler and a fiber pinned to its home (no migration yet), a
// tightly-coupled pair spawned from a non-fiber can land on different
// schedulers and pay a ~5 us cross-thread wakeup per message instead of a
// ~30 ns register switch (a ~16x regression on fine-grained ping-pong). So
// multi-scheduler is OPT-IN via PLUTO_GREEN_SCHEDULERS=N, which buys CPU-bound
// green fan-out ~5x parallelism without regressing the default. Flipping the
// default to one-per-core awaits work-stealing / wake-locality (safe migration).
static int desired_nsched(void) {
    const char *env = getenv("PLUTO_GREEN_SCHEDULERS");
    int n;
    if (env && (n = atoi(env)) > 0) {
        return n > MAX_SCHED ? MAX_SCHED : n;
    }
    return 1;
}

static void gprod_ensure_started(void) {
    pthread_mutex_lock(&g_start_mu);
    if (!g_started) {
        g_started = 1;
        g_wake_local = getenv("PLUTO_GREEN_WAKE_LOCALITY") != NULL;
        g_sp_check = getenv("PLUTO_GREEN_SP_CHECK") != NULL;
        g_nsched = desired_nsched();
        for (int i = 0; i < g_nsched; i++) {
            GProdSched *s = &g_scheds[i];
            pthread_mutex_init(&s->mu, NULL);
            pthread_cond_init(&s->wake, NULL);
            s->head = s->tail = s->timer_head = NULL;
            s->local_head = s->local_tail = NULL;
            s->sched_sp = NULL; s->current = NULL;
            pthread_create(&s->thread, NULL, gprod_scheduler, s);
        }
    }
    pthread_mutex_unlock(&g_start_mu);
}

// Choose a home scheduler for a new task. PARENT-LOCALITY: a task spawned by a
// fiber inherits that fiber's scheduler, so a fiber and the sub-tasks it
// spawns-and-talks-to stay co-located (handoffs are register switches, no
// cross-thread wakeups). A task spawned by a NON-fiber (main / a spawn thread)
// has no parent scheduler, so it goes round-robin — spreading top-level fan-out
// across cores for parallelism. Since a fiber never migrates, its home is
// fixed here for life.
static GProdSched *pick_sched(void) {
    if (cur_sched) return cur_sched;   // parent-locality: child joins parent
    unsigned long i = __atomic_fetch_add(&g_rr, 1, __ATOMIC_RELAXED);
    return &g_scheds[i % (unsigned)g_nsched];
}

static GProdTask *gprod_alloc(size_t stack_size) {
    GProdTask *t = (GProdTask *)calloc(1, sizeof(GProdTask));
    // Fiber stacks grow DOWN, so a one-page PROT_NONE guard at the LOW end
    // turns a stack overflow into an immediate fault instead of silent
    // corruption of whatever abutted the old malloc'd block.
    long pg = sysconf(_SC_PAGESIZE);
    size_t guard = (pg > 0) ? (size_t)pg : 16384;
    size_t total = guard + stack_size;
    char *map = (char *)mmap(NULL, total, PROT_READ | PROT_WRITE,
                             MAP_PRIVATE | MAP_ANON, -1, 0);
    if (map == MAP_FAILED) { abort(); }
    mprotect(map, guard, PROT_NONE);          // low guard page
    t->stack_map = map;
    t->stack_map_size = total;
    t->stack = map + guard;
    t->stack_size = stack_size;
    t->sp = prime_ctx(t->stack + stack_size, gprod_trampoline);
    t->state = GRUN;
    return t;
}

GProdTask *__pluto_green_prod_spawn(long (*fn)(void *), void *arg, size_t stack_size) {
    gprod_ensure_started();
    GProdTask *t = gprod_alloc(stack_size);
    t->fn = fn; t->arg = arg;
    pthread_mutex_init(&t->mu, NULL);
    pthread_cond_init(&t->cv, NULL);
    t->home = pick_sched();
    sched_push_remote(t->home, t);
    return t;
}

long __pluto_green_prod_get(GProdTask *t) {
    pthread_mutex_lock(&t->mu);
    while (!t->done) pthread_cond_wait(&t->cv, &t->mu);
    long r = t->result;
    pthread_mutex_unlock(&t->mu);
    return r;
}

// Submit a green task that runs `run(job)` to completion on its home scheduler,
// GC-context registered. Result delivery is the callback's job. The scheduler
// owns and frees the fiber. Entry point for threading.c's __pluto_green_spawn.
void __pluto_green_submit(void (*run)(void *), void *job, size_t stack_size) {
    gprod_ensure_started();
    GProdTask *t = gprod_alloc(stack_size);
    t->submit_run = run;
    t->submit_job = job;
    t->home = pick_sched();
    sched_push_remote(t->home, t);
}
#endif
