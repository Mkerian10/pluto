// cp6 standalone proof: the scheduler timer wheel. Green fibers that sleep yield
// the scheduler to peers and wake in deadline order, concurrently (not serial).
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

typedef struct GProdTask GProdTask;
GProdTask *__pluto_green_prod_spawn(long (*fn)(void *), void *arg, size_t stack);
long __pluto_green_prod_get(GProdTask *t);
void __pluto_green_sleep_ns(long ns);

void *__pluto_gc_register_green_context(void *t, void *s) { (void)t; (void)s; return (void *)1; }
void __pluto_gc_unregister_green_context(void *h) { (void)h; }
void __pluto_gc_register_thread_stack(void *lo, void *hi) { (void)lo; (void)hi; }
void __pluto_gc_enter_safe_region(void) {}
void __pluto_gc_leave_safe_region(void) {}

#define STK (256 * 1024)
static int failures = 0;
#define CHECK(c, m) do { if (!(c)) { printf("FAIL: %s\n", m); failures++; } } while (0)

static long now_ms(void) { struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts); return ts.tv_sec*1000L + ts.tv_nsec/1000000L; }
static long t0;
static int order[8]; static int order_n;

static long sleeper(void *arg) {
    long ms = (long)arg;
    __pluto_green_sleep_ns(ms * 1000000L);
    order[order_n++] = (int)ms;     // single scheduler thread: no race
    return now_ms() - t0;            // woke_at ms
}

int main(void) {
    t0 = now_ms();
    // Spawn out of deadline order: 150, 50, 100.
    GProdTask *a = __pluto_green_prod_spawn(sleeper, (void *)150, STK);
    GProdTask *b = __pluto_green_prod_spawn(sleeper, (void *)50, STK);
    GProdTask *c = __pluto_green_prod_spawn(sleeper, (void *)100, STK);
    long wa = __pluto_green_prod_get(a);
    long wb = __pluto_green_prod_get(b);
    long wc = __pluto_green_prod_get(c);
    long total = now_ms() - t0;

    CHECK(order_n == 3, "all three slept and woke");
    CHECK(order[0] == 50 && order[1] == 100 && order[2] == 150, "woke in deadline order");
    CHECK(wb >= 40 && wb <= 90, "50ms sleeper woke ~50ms");
    CHECK(wc >= 90 && wc <= 150, "100ms sleeper woke ~100ms");
    CHECK(wa >= 140 && wa <= 210, "150ms sleeper woke ~150ms");
    CHECK(total >= 140 && total <= 220, "concurrent: total ~150ms, not ~300ms serial");

    if (failures == 0) printf("cp6 timer: ALL PASS (order=%d,%d,%d wa=%ld wb=%ld wc=%ld total=%ld)\n", order[0],order[1],order[2], wa,wb,wc, total);
    return failures ? 1 : 0;
}
