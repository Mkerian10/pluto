// Cooperative green channel (#369 Phase 3 prototype): a bounded buffer whose
// recv parks the running green task when empty and whose send parks when full,
// each waking one peer from the other side. Single-threaded within a scheduler
// — no locks. Mirrors the production channel's semantics (FIFO, bounded) but
// blocking becomes a scheduler yield, not an OS-thread block.
#ifndef PLUTO_GREEN_CHAN_H
#define PLUTO_GREEN_CHAN_H
typedef struct GChan GChan;
GChan *gchan_new(long cap);
void   gchan_send(GChan *c, long v);
long   gchan_recv(GChan *c);
#endif
