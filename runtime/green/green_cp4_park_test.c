// cp4 standalone proof: cooperative PARK / YIELD / WAKE in the production green
// scheduler, with no real GC (stubs) — the concurrency core in isolation.
//
// Builds: cc -O2 green_cp4_park_test.c green_prod.c ctx_<arch>.s -lpthread
//
// Proves:
//   A. Two fibers hand a baton back and forth via park/wake and interleave
//      deterministically on ONE scheduler thread (fiber-to-fiber register
//      switch). Done as an explicit ping-pong so there is no startup race.
//   B. A fiber that parks is woken cross-thread (main fires the event after the
//      fiber has suspended) and resumes with the right result — the scheduler
//      kept serving in between, it did not block.
//   C. The already-fired fast path: parking on an event that already has a
//      token re-readies the fiber immediately (no lost wakeup, no hang).
//
// The Event here is a binary semaphore: fire hands a token directly to a parked
// waiter, or leaves the token; wait consumes a token or parks for one.
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

// ── scheduler API under test ─────────────────────────────────────────────────
typedef struct GProdTask GProdTask;
GProdTask *__pluto_green_prod_spawn(long (*fn)(void *), void *arg, size_t stack);
long __pluto_green_prod_get(GProdTask *t);
void *__pluto_green_self(void);
void __pluto_green_yield(void);
void __pluto_green_park(void (*handoff)(void *), void *arg);
void __pluto_green_wake(void *fiber);

// ── GC stubs (this test has no collector) ────────────────────────────────────
void *__pluto_gc_register_green_context(void *top, void *sp) { (void)top; (void)sp; return (void *)1; }
void __pluto_gc_unregister_green_context(void *h) { (void)h; }
void __pluto_gc_register_thread_stack(void *lo, void *hi) { (void)lo; (void)hi; }
void __pluto_gc_enter_safe_region(void) {}
void __pluto_gc_leave_safe_region(void) {}

#define STK (256 * 1024)
static int failures = 0;
#define CHECK(c, msg) do { if (!(c)) { printf("FAIL: %s\n", msg); failures++; } } while (0)

// ── Event: binary semaphore built on park/wake ───────────────────────────────
typedef struct { pthread_mutex_t m; int token; void *waiter; } Event;
typedef struct { Event *e; void *self; } ParkArg;

static void event_handoff(void *p) {
    ParkArg *pa = (ParkArg *)p;
    pthread_mutex_lock(&pa->e->m);
    if (pa->e->token) {                      // token already present: consume it
        pa->e->token = 0;
        pthread_mutex_unlock(&pa->e->m);
        __pluto_green_wake(pa->self);        // re-ready now (no lost wakeup)
    } else {
        pa->e->waiter = pa->self;            // publish as waiter, stay parked
        pthread_mutex_unlock(&pa->e->m);
    }
}

static void event_wait(Event *e) {
    ParkArg pa = { e, __pluto_green_self() };
    __pluto_green_park(event_handoff, &pa);  // resumes here once it has a token
}

static void event_fire(Event *e) {
    pthread_mutex_lock(&e->m);
    void *w = e->waiter;
    if (w) { e->waiter = NULL; pthread_mutex_unlock(&e->m); __pluto_green_wake(w); }
    else   { e->token = 1;     pthread_mutex_unlock(&e->m); }
}

static void event_init(Event *e) { memset(e, 0, sizeof *e); pthread_mutex_init(&e->m, NULL); }

// ── A. ping-pong baton between two fibers ─────────────────────────────────────
#define PINGS 10
static Event ping_ev, pong_ev;
static int trace[2 * PINGS];
static int trace_n = 0;

static long pinger(void *arg) {            // records 0, hands to pong, waits
    (void)arg;
    for (int i = 0; i < PINGS; i++) {
        event_wait(&ping_ev);
        trace[trace_n++] = 0;
        event_fire(&pong_ev);
    }
    return 0;
}
static long ponger(void *arg) {            // records 1, hands to ping, waits
    (void)arg;
    for (int i = 0; i < PINGS; i++) {
        event_wait(&pong_ev);
        trace[trace_n++] = 1;
        event_fire(&ping_ev);
    }
    return 1;
}

// ── B / C. park + wake ───────────────────────────────────────────────────────
static Event ev_B, ev_C;
static long parker_B(void *arg) { event_wait((Event *)arg); return 0xB; }
static long parker_C(void *arg) {
    __pluto_green_yield();                 // run the scheduler once first
    event_wait((Event *)arg);              // event already has a token
    return 0xC;
}

int main(void) {
    // A: deterministic alternation via an explicit baton.
    event_init(&ping_ev); event_init(&pong_ev);
    GProdTask *pg = __pluto_green_prod_spawn(pinger, NULL, STK);
    GProdTask *po = __pluto_green_prod_spawn(ponger, NULL, STK);
    usleep(20000);                         // let both reach their first wait
    event_fire(&ping_ev);                  // start the volley
    CHECK(__pluto_green_prod_get(pg) == 0, "pinger finished");
    CHECK(__pluto_green_prod_get(po) == 1, "ponger finished");
    CHECK(trace_n == 2 * PINGS, "all baton passes recorded");
    int ok = (trace_n == 2 * PINGS);
    for (int i = 0; i < trace_n; i++) if (trace[i] != (i & 1 ? 1 : 0)) ok = 0;
    CHECK(ok, "fibers strictly alternated 0,1,0,1 (fiber-to-fiber switch)");

    // B: park, then fire from the main thread after the fiber has suspended.
    event_init(&ev_B);
    GProdTask *b = __pluto_green_prod_spawn(parker_B, &ev_B, STK);
    usleep(20000);
    pthread_mutex_lock(&ev_B.m);
    int parked = (ev_B.waiter != NULL);     // read under the lock that guards it
    pthread_mutex_unlock(&ev_B.m);
    CHECK(parked, "fiber parked and published itself as waiter");
    event_fire(&ev_B);                     // cross-thread wake
    CHECK(__pluto_green_prod_get(b) == 0xB, "parked fiber woken cross-thread");

    // C: leave a token BEFORE the fiber waits — the already-fired fast path.
    event_init(&ev_C);
    event_fire(&ev_C);
    GProdTask *c = __pluto_green_prod_spawn(parker_C, &ev_C, STK);
    CHECK(__pluto_green_prod_get(c) == 0xC, "already-fired park did not hang");

    if (failures == 0) printf("cp4 park/yield/wake: ALL PASS (trace_n=%d)\n", trace_n);
    return failures ? 1 : 0;
}
