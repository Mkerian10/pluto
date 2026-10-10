#include "green_chan.h"
#include "green_sched.h"
#include <stdlib.h>

// Parked-waiter list (a small dynamic array of task handles; a task waits on at
// most one channel at a time). Single-threaded within a scheduler — no locks.
typedef struct { GTask **v; long n, cap; } Vec;
static void vec_push(Vec *q, GTask *t) {
    if (q->n == q->cap) { q->cap = q->cap ? q->cap * 2 : 8; q->v = realloc(q->v, q->cap * sizeof(GTask *)); }
    q->v[q->n++] = t;
}
static GTask *vec_shift(Vec *q) {
    if (q->n == 0) return 0;
    GTask *t = q->v[0];
    for (long i = 1; i < q->n; i++) q->v[i - 1] = q->v[i];
    q->n--;
    return t;
}

struct GChan { long *buf, cap, count, head, tail; Vec recvq, sendq; };

GChan *gchan_new(long cap) {
    GChan *c = calloc(1, sizeof(GChan));
    c->cap = cap > 0 ? cap : 1;
    c->buf = calloc(c->cap, sizeof(long));
    return c;
}
void gchan_send(GChan *c, long v) {
    while (c->count == c->cap) { vec_push(&c->sendq, green_current()); green_park(); }
    c->buf[c->tail] = v; c->tail = (c->tail + 1) % c->cap; c->count++;
    GTask *r = vec_shift(&c->recvq); if (r) green_wake(r);
}
long gchan_recv(GChan *c) {
    while (c->count == 0) { vec_push(&c->recvq, green_current()); green_park(); }
    long v = c->buf[c->head]; c->head = (c->head + 1) % c->cap; c->count--;
    GTask *s = vec_shift(&c->sendq); if (s) green_wake(s);
    return v;
}
