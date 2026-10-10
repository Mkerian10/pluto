// Minimal single-threaded cooperative green scheduler (#369 Phase 2 prototype).
// One scheduler == one OS thread running many green tasks cooperatively via the
// register-only context switch (ctx_*.s). NOT yet wired into the runtime build
// or GC; a standalone, testable core for the mechanism.
#ifndef PLUTO_GREEN_SCHED_H
#define PLUTO_GREEN_SCHED_H
#include <stddef.h>
typedef struct GTask GTask;
// Spawn a green task running fn(arg) on a fresh `stack_size`-byte stack.
GTask *green_spawn(void (*fn)(void *), void *arg, size_t stack_size);
// Cooperatively yield the running green task back to the scheduler.
void green_yield(void);
// Run the scheduler until every green task has finished. Returns on an empty
// run queue. Call from the OS thread that owns this scheduler.
void green_run(void);
// Number of green tasks created (for test assertions).
long green_task_count(void);
#endif

// ── Park / wake (Phase 3): cooperative blocking ──────────────────────────────
// green_park() suspends the running task WITHOUT re-queueing it (unlike
// green_yield) and returns to the scheduler; the task resumes only when some
// other task passes its handle to green_wake(). This is how a green task
// blocks on a channel/event without consuming a scheduler slot while idle.
void green_park(void);
void green_wake(GTask *t);
GTask *green_current(void);
long green_live(void);   // tasks created but not yet finished (incl. parked)
void green_task_live_range(GTask *t, void **lo, void **hi); // [sp, top) to GC-scan
