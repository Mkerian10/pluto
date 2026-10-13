// Isolate the wake-locality migration SEGFAULT (#369 multi-scheduler): move a
// parked fiber from its home scheduler to the waker's scheduler and resume it
// there, with NO Pluto codegen involved — just the scheduler + the register
// context switch. If this crashes, the bug is in the scheduler mechanism; if it
// runs, the bug was in the generated code's thread assumptions.
//
// Build: cc -O1 -g green_migrate_test.c green_prod.c ctx_arm64.s -lpthread
// Run:   PLUTO_GREEN_SCHEDULERS=2 PLUTO_GREEN_WAKE_LOCALITY=1 \
//        PLUTO_GREEN_SP_CHECK=1 ./a.out
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef struct GProdTask GProdTask;
GProdTask *__pluto_green_prod_spawn(long (*fn)(void *), void *arg, size_t stack);
long __pluto_green_prod_get(GProdTask *t);
void *__pluto_green_self(void);
void __pluto_green_park(void (*handoff)(void *), void *arg);
void __pluto_green_wake(void *fiber);

void *__pluto_gc_register_green_context(void *t, void *s) { (void)t; (void)s; return (void *)1; }
__thread void *__pluto_current_error = 0;
__thread void *__pluto_current_error_type = 0;
__thread long *__pluto_current_task = 0;
void __pluto_gc_green_set_live_sp(void *h, void *sp) { (void)h; (void)sp; }
void __pluto_gc_unregister_green_context(void *h) { (void)h; }
void __pluto_gc_register_thread_stack(void *lo, void *hi) { (void)lo; (void)hi; }
void __pluto_gc_enter_safe_region(void) {}
void __pluto_gc_leave_safe_region(void) {}

#define STK (256 * 1024)
static int failures = 0;
#define CHECK(c, m) do { if (!(c)) { printf("FAIL: %s\n", m); failures++; } } while (0)

// Binary-semaphore event, like the cp4 test.
typedef struct { pthread_mutex_t m; int token; void *waiter; } Event;
typedef struct { Event *e; void *self; } ParkArg;
static void ev_handoff(void *p) {
    ParkArg *pa = (ParkArg *)p;
    pthread_mutex_lock(&pa->e->m);
    if (pa->e->token) { pa->e->token = 0; pthread_mutex_unlock(&pa->e->m); __pluto_green_wake(pa->self); }
    else { pa->e->waiter = pa->self; pthread_mutex_unlock(&pa->e->m); }
}
static void ev_wait(Event *e) { ParkArg pa = { e, __pluto_green_self() }; __pluto_green_park(ev_handoff, &pa); }
static void ev_fire(Event *e) {
    pthread_mutex_lock(&e->m);
    void *w = e->waiter;
    if (w) { e->waiter = NULL; pthread_mutex_unlock(&e->m); __pluto_green_wake(w); }
    else   { e->token = 1;     pthread_mutex_unlock(&e->m); }
}
static void ev_init(Event *e) { memset(e, 0, sizeof *e); pthread_mutex_init(&e->m, NULL); }

// Recurse to stress the fiber's stack AFTER it resumes post-migration: if the
// resumed context is corrupt this writes/returns through garbage and faults.
static long deep_sum(long n) { if (n <= 0) return 0; return n + deep_sum(n - 1); }

static Event ev;

// The migrating fiber: records which thread it starts on, parks, then (after
// being woken onto the waker's scheduler) records the thread it resumes on and
// does stack work. If migration works, start_tid != resume_tid.
static volatile unsigned long start_tid, resume_tid;
static long migrant(void *arg) {
    (void)arg;
    start_tid = (unsigned long)pthread_self();
    ev_wait(&ev);                       // park on home scheduler
    resume_tid = (unsigned long)pthread_self();
    return deep_sum(10000);             // 50005000 — stresses the resumed stack
}

// The waker fiber: fires the event (waking the migrant onto THIS fiber's
// scheduler). Spun up after the migrant has parked.
static long waker(void *arg) {
    (void)arg;
    ev_fire(&ev);
    return deep_sum(5000);              // 12502500
}

int main(void) {
    ev_init(&ev);
    GProdTask *m = __pluto_green_prod_spawn(migrant, NULL, STK);  // -> scheduler 0
    usleep(50000);                       // let the migrant reach its park
    GProdTask *w = __pluto_green_prod_spawn(waker, NULL, STK);    // -> scheduler 1
    long mr = __pluto_green_prod_get(m);
    long wr = __pluto_green_prod_get(w);
    CHECK(mr == 50005000L, "migrant resumed with intact stack");
    CHECK(wr == 12502500L, "waker ran");
    CHECK(start_tid != 0 && resume_tid != 0, "migrant ran before and after park");
    if (failures == 0)
        printf("migrate: ALL PASS (start_tid=%lu resume_tid=%lu migrated=%s)\n",
               start_tid, resume_tid, start_tid == resume_tid ? "no" : "YES");
    return failures ? 1 : 0;
}
