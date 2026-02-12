//──────────────────────────────────────────────────────────────────────────────
// Pluto Runtime: Concurrency
//
// Task-based concurrency primitives (spawn, channels, select).
//
// Design:
// - Test mode: Cooperative fiber scheduler with exhaustive DPOR state exploration
// - Production mode: Pthread-based tasks with mutex-protected channels
// - Deep copy semantics for spawn arguments (value isolation between tasks)
// - Rwlock synchronization for contract enforcement on shared objects
//
// API:
// - Tasks: __pluto_task_spawn, __pluto_task_get, __pluto_task_detach, __pluto_task_cancel
// - Channels: __pluto_chan_create, __pluto_chan_send, __pluto_chan_recv, __pluto_chan_close
// - Select: __pluto_select_init, __pluto_select_add_recv, __pluto_select_add_send, __pluto_select_wait
// - Sync: __pluto_rwlock_* (for contract invariants on concurrent objects)
//──────────────────────────────────────────────────────────────────────────────

#include "builtins.h"

// ── Concurrency ─────────────────────────────────────────────────────────────

// Task handle layout (56 bytes, 7 slots):
//   [0] closure   (i64, GC pointer)
//   [1] result    (i64)
//   [2] error     (i64, GC pointer)
//   [3] done      (i64)
//   [4] sync_ptr  (i64, raw malloc — NULL in test mode)
//   [5] detached  (i64, 0 or 1)
//   [6] cancelled (i64, 0 or 1)

static void task_raise_cancelled(void) {
    const char *msg = "task cancelled";
    void *msg_str = __pluto_string_new((char *)msg, (long)strlen(msg));
    void *err_obj = __pluto_alloc(8);  // 1 field: message
    *(long *)err_obj = (long)msg_str;
    __pluto_raise_error(err_obj);
}

#ifdef PLUTO_TEST_MODE

// ── Fiber scheduler infrastructure ──────────────────────────────────────────

#define FIBER_STACK_SIZE (64 * 1024)   // 64KB per fiber stack
#define MAX_FIBERS 256

typedef enum { STRATEGY_SEQUENTIAL=0, STRATEGY_ROUND_ROBIN=1, STRATEGY_RANDOM=2, STRATEGY_EXHAUSTIVE=3 } Strategy;
typedef enum {
    FIBER_READY=0, FIBER_RUNNING=1,
    FIBER_BLOCKED_TASK=2, FIBER_BLOCKED_CHAN_SEND=3,
    FIBER_BLOCKED_CHAN_RECV=4, FIBER_BLOCKED_SELECT=5,
    FIBER_COMPLETED=6
} FiberState;

typedef struct {
    ucontext_t context;
    char *stack;
    FiberState state;
    long *task;              // associated task handle (NULL for fiber 0 / main test fiber)
    long closure_ptr;        // closure to execute (for spawned fibers)
    void *blocked_on;        // task handle or channel handle we're waiting on
    long blocked_value;      // value for pending send
    int id;
    // Per-fiber saved TLS state (restored on context switch)
    void *saved_error;       // __pluto_current_error
    long *saved_current_task; // __pluto_current_task
} Fiber;

typedef struct {
    Fiber fibers[MAX_FIBERS];
    int fiber_count;
    int current_fiber;
    Strategy strategy;
    uint64_t seed;
    long main_fn_ptr;        // test function pointer (fiber 0 entry)
    ucontext_t scheduler_ctx;
    int deadlock;
} Scheduler;

static Scheduler *g_scheduler = NULL;

// ── Exhaustive (DPOR) state ─────────────────────────────────────────────────

#define EXHST_MAX_DEPTH 200
#define EXHST_MAX_CHANNELS_PER_FIBER 32
#define EXHST_MAX_FAILURES 64

typedef struct {
    // Current schedule trace
    int choices[EXHST_MAX_DEPTH];                  // fiber index chosen at each yield point
    int ready[EXHST_MAX_DEPTH][MAX_FIBERS];        // ready fibers at each yield point
    int ready_count[EXHST_MAX_DEPTH];              // count of ready fibers at each yield
    int depth;                                      // current yield point index

    // Replay state for backtracking
    int replay_prefix[EXHST_MAX_DEPTH];            // choices to replay
    int replay_len;                                 // how many choices to replay
    int replay_next_choice;                         // forced choice after replay

    // DPOR: channel dependency tracking per fiber (per-schedule, reset each run)
    void *fiber_channels[MAX_FIBERS][EXHST_MAX_CHANNELS_PER_FIBER];
    int fiber_channel_count[MAX_FIBERS];

    // DPOR: accumulated dependency matrix (persistent across schedules)
    int dep_matrix[MAX_FIBERS][MAX_FIBERS];        // 1 = fibers share channels
    int dep_valid;                                  // 1 once first schedule observed

    // Bookkeeping
    int schedules_explored;
    int max_schedules;
    int max_depth;
    int fiber_count_snapshot;                       // fiber count for dep matrix update

    // Failure collection
    int failure_count;
    char *failure_messages[EXHST_MAX_FAILURES];
} ExhaustiveState;

static ExhaustiveState *g_exhaustive = NULL;

// Forward declarations for fiber scheduler
static void scheduler_run(void);
static void fiber_yield_to_scheduler(void);
static void test_main_fiber_entry(void);

// ── Fiber helper functions ──────────────────────────────────────────────────

static void wake_fibers_blocked_on_task(long *task_ptr) {
    if (!g_scheduler) return;
    for (int i = 0; i < g_scheduler->fiber_count; i++) {
        Fiber *f = &g_scheduler->fibers[i];
        if (f->state == FIBER_BLOCKED_TASK && f->blocked_on == (void *)task_ptr) {
            f->state = FIBER_READY;
            f->blocked_on = NULL;
        }
    }
}

static void wake_fibers_blocked_on_chan(long *ch_ptr) {
    if (!g_scheduler) return;
    for (int i = 0; i < g_scheduler->fiber_count; i++) {
        Fiber *f = &g_scheduler->fibers[i];
        if ((f->state == FIBER_BLOCKED_CHAN_SEND || f->state == FIBER_BLOCKED_CHAN_RECV ||
             f->state == FIBER_BLOCKED_SELECT) && f->blocked_on == (void *)ch_ptr) {
            f->state = FIBER_READY;
            f->blocked_on = NULL;
        }
    }
}

// Wake ALL fibers blocked on select that include this channel in their buffer
static void wake_select_fibers_for_chan(long *ch_ptr) {
    if (!g_scheduler) return;
    for (int i = 0; i < g_scheduler->fiber_count; i++) {
        Fiber *f = &g_scheduler->fibers[i];
        if (f->state == FIBER_BLOCKED_SELECT) {
            // For select, blocked_on points to the buffer_ptr array
            // We wake unconditionally since we can't cheaply check all handles
            f->state = FIBER_READY;
            f->blocked_on = NULL;
        }
    }
}

static uint64_t lcg_next(uint64_t *seed) {
    *seed = (*seed) * 6364136223846793005ULL + 1442695040888963407ULL;
    return *seed;
}

// ── Exhaustive helper functions ─────────────────────────────────────────────

static void exhaustive_record_channel(int fiber_id, void *channel) {
    if (!g_exhaustive) return;
    ExhaustiveState *es = g_exhaustive;
    if (fiber_id < 0 || fiber_id >= MAX_FIBERS) return;
    // Deduplicate
    for (int i = 0; i < es->fiber_channel_count[fiber_id]; i++) {
        if (es->fiber_channels[fiber_id][i] == channel) return;
    }
    if (es->fiber_channel_count[fiber_id] < EXHST_MAX_CHANNELS_PER_FIBER) {
        es->fiber_channels[fiber_id][es->fiber_channel_count[fiber_id]++] = channel;
    }
}

static void exhaustive_update_dep_matrix(ExhaustiveState *es, int fiber_count) {
    // After a complete schedule, update the dependency matrix.
    // Two fibers are dependent if they share at least one channel.
    for (int a = 0; a < fiber_count; a++) {
        for (int b = a + 1; b < fiber_count; b++) {
            int shared = 0;
            for (int ci = 0; ci < es->fiber_channel_count[a] && !shared; ci++) {
                for (int cj = 0; cj < es->fiber_channel_count[b] && !shared; cj++) {
                    if (es->fiber_channels[a][ci] == es->fiber_channels[b][cj])
                        shared = 1;
                }
            }
            if (shared) {
                es->dep_matrix[a][b] = 1;
                es->dep_matrix[b][a] = 1;
            }
        }
    }
    es->dep_valid = 1;
}

static int exhaustive_find_backtrack(ExhaustiveState *es) {
    // Walk backward through yield points to find an unexplored alternative.
    // With DPOR: skip alternatives that are independent of the chosen fiber.
    for (int i = es->depth - 1; i >= 0; i--) {
        int chosen = es->choices[i];
        int *rdy = es->ready[i];
        int rc = es->ready_count[i];

        if (rc <= 1) continue;  // only one choice at this yield point

        // Find position of chosen fiber in the ready set
        int pos = -1;
        for (int j = 0; j < rc; j++) {
            if (rdy[j] == chosen) { pos = j; break; }
        }
        if (pos < 0 || pos >= rc - 1) continue;  // no more alternatives

        // Try subsequent alternatives
        for (int j = pos + 1; j < rc; j++) {
            int alt = rdy[j];
            // DPOR pruning: skip if we know they're independent
            if (es->dep_valid && !es->dep_matrix[chosen][alt]) continue;

            // Found a viable backtrack point
            memcpy(es->replay_prefix, es->choices, i * sizeof(int));
            es->replay_len = i;
            es->replay_next_choice = alt;
            return 1;
        }
    }
    return 0;  // all schedules explored
}

static int pick_next_fiber(void) {
    if (!g_scheduler) return -1;
    int n = g_scheduler->fiber_count;

    if (g_scheduler->strategy == STRATEGY_ROUND_ROBIN) {
        // Round-robin: start from current+1, find first READY
        for (int off = 1; off <= n; off++) {
            int idx = (g_scheduler->current_fiber + off) % n;
            if (g_scheduler->fibers[idx].state == FIBER_READY) return idx;
        }
        return -1;
    } else if (g_scheduler->strategy == STRATEGY_EXHAUSTIVE && g_exhaustive) {
        // Exhaustive: DFS over schedule tree with DPOR pruning
        ExhaustiveState *es = g_exhaustive;

        // Collect ready fibers
        int ready[MAX_FIBERS];
        int ready_count = 0;
        for (int i = 0; i < n; i++) {
            if (g_scheduler->fibers[i].state == FIBER_READY) {
                ready[ready_count++] = i;
            }
        }
        if (ready_count == 0) return -1;

        if (es->depth >= es->max_depth) {
            // Past depth limit: pick first ready without recording
            return ready[0];
        }

        // Record the ready set at this yield point
        memcpy(es->ready[es->depth], ready, ready_count * sizeof(int));
        es->ready_count[es->depth] = ready_count;

        int choice;
        if (es->depth < es->replay_len) {
            // Replaying a prefix: use the predetermined choice
            choice = es->replay_prefix[es->depth];
        } else if (es->depth == es->replay_len && es->replay_next_choice >= 0) {
            // First new choice after replay: use the forced alternative
            choice = es->replay_next_choice;
            es->replay_next_choice = -1;
        } else {
            // New territory: pick the first ready fiber (DFS order)
            choice = ready[0];
        }

        es->choices[es->depth] = choice;
        es->depth++;
        return choice;
    } else {
        // Random: collect all READY fibers, pick one using LCG
        int ready[MAX_FIBERS];
        int ready_count = 0;
        for (int i = 0; i < n; i++) {
            if (g_scheduler->fibers[i].state == FIBER_READY) {
                ready[ready_count++] = i;
            }
        }
        if (ready_count == 0) return -1;
        uint64_t r = lcg_next(&g_scheduler->seed);
        return ready[(int)(r % (uint64_t)ready_count)];
    }
}

static int all_fibers_done(void) {
    for (int i = 0; i < g_scheduler->fiber_count; i++) {
        if (g_scheduler->fibers[i].state != FIBER_COMPLETED) return 0;
    }
    return 1;
}

static void fiber_yield_to_scheduler(void) {
    int cur = g_scheduler->current_fiber;
    Fiber *f = &g_scheduler->fibers[cur];
    // Save TLS state
    f->saved_error = __pluto_current_error;
    f->saved_current_task = __pluto_current_task;
    swapcontext(&f->context, &g_scheduler->scheduler_ctx);
    // Resumed — TLS state restored by scheduler before switching to us
}

static void fiber_entry_fn(int fiber_id) {
    Fiber *f = &g_scheduler->fibers[fiber_id];
    long *task = f->task;

    // Execute the closure
    long fn_ptr = *(long *)f->closure_ptr;
    long result = ((long(*)(long))fn_ptr)(f->closure_ptr);

    // Store result or error in task handle
    if (__pluto_current_error) {
        task[2] = (long)__pluto_current_error;
        __pluto_current_error = NULL;
    } else {
        task[1] = result;
    }
    task[3] = 1;  // done

    // If detached and errored, print to stderr
    if (task[5] && task[2]) {
        long *err_obj = (long *)task[2];
        char *msg_ptr = (char *)err_obj[0];
        if (msg_ptr) {
            long len = *(long *)msg_ptr;
            char *data = msg_ptr + 8;
            fprintf(stderr, "pluto: error in detached task: %.*s\n", (int)len, data);
        }
    }

    f->state = FIBER_COMPLETED;

    // Wake any fibers waiting on this task
    wake_fibers_blocked_on_task(task);

    // Return to scheduler via uc_link
}

static void test_main_fiber_entry(void) {
    // Execute the test function (no closure env, just a plain function pointer)
    ((void(*)(void))g_scheduler->main_fn_ptr)();
    g_scheduler->fibers[0].state = FIBER_COMPLETED;
    // Return to scheduler via uc_link
}

static void scheduler_run(void) {
    while (1) {
        int next = pick_next_fiber();
        if (next == -1) {
            if (all_fibers_done()) break;
            // Deadlock: all remaining fibers are blocked
            fprintf(stderr, "pluto: deadlock detected in test\n");
            for (int i = 0; i < g_scheduler->fiber_count; i++) {
                Fiber *f = &g_scheduler->fibers[i];
                if (f->state >= FIBER_BLOCKED_TASK && f->state <= FIBER_BLOCKED_SELECT) {
                    const char *reason = "unknown";
                    switch (f->state) {
                        case FIBER_BLOCKED_TASK:      reason = "task.get()"; break;
                        case FIBER_BLOCKED_CHAN_SEND:  reason = "chan.send()"; break;
                        case FIBER_BLOCKED_CHAN_RECV:  reason = "chan.recv()"; break;
                        case FIBER_BLOCKED_SELECT:     reason = "select"; break;
                        default: break;
                    }
                    fprintf(stderr, "  Fiber %d: blocked on %s\n", i, reason);
                }
            }
            g_scheduler->deadlock = 1;
            break;
        }

        // Restore next fiber's TLS state
        g_scheduler->current_fiber = next;
        __pluto_gc_set_current_fiber(next);  // Tell GC which fiber is running
        Fiber *f = &g_scheduler->fibers[next];
        __pluto_current_error = f->saved_error;
        __pluto_current_task = f->saved_current_task;
        f->state = FIBER_RUNNING;

        swapcontext(&g_scheduler->scheduler_ctx, &f->context);

        __pluto_gc_set_current_fiber(-1);  // Back in scheduler context

        // Fiber yielded or completed — state already saved in fiber_yield_to_scheduler
        // (or fiber completed and returned via uc_link)
        // For completed fibers that return via uc_link, save their state too
        Fiber *yielded = &g_scheduler->fibers[g_scheduler->current_fiber];
        if (yielded->state != FIBER_COMPLETED) {
            // State was saved by fiber_yield_to_scheduler already
        } else {
            yielded->saved_error = __pluto_current_error;
            yielded->saved_current_task = __pluto_current_task;
            __pluto_gc_mark_fiber_complete(g_scheduler->current_fiber);
        }
    }
}

// ── __pluto_test_run: entry point called by codegen ──

// Helper: create a fresh scheduler with fiber 0 and run it.
// Returns 1 if deadlock occurred, 0 otherwise.
static int test_run_single(long fn_ptr, Strategy strategy, uint64_t run_seed) {
    g_scheduler = (Scheduler *)calloc(1, sizeof(Scheduler));
    g_scheduler->strategy = strategy;
    g_scheduler->seed = run_seed;
    g_scheduler->main_fn_ptr = fn_ptr;

    // Create fiber 0 for the test body
    Fiber *f = &g_scheduler->fibers[0];
    f->id = 0;
    f->state = FIBER_READY;
    f->stack = (char *)malloc(FIBER_STACK_SIZE);
    f->task = NULL;
    f->closure_ptr = 0;
    f->saved_error = NULL;
    f->saved_current_task = NULL;
    getcontext(&f->context);
    f->context.uc_stack.ss_sp = f->stack;
    f->context.uc_stack.ss_size = FIBER_STACK_SIZE;
    f->context.uc_link = &g_scheduler->scheduler_ctx;
    makecontext(&f->context, (void(*)(void))test_main_fiber_entry, 0);
    g_scheduler->fiber_count = 1;

    // Register fiber 0 with GC fiber stack scanner
    __pluto_gc_register_fiber_stack(f->stack, FIBER_STACK_SIZE);
    __pluto_gc_set_current_fiber(-1);
    __pluto_gc_enable_fiber_scanning();

    scheduler_run();

    __pluto_gc_disable_fiber_scanning();
    int had_deadlock = g_scheduler->deadlock;
    int fiber_count = g_scheduler->fiber_count;

    for (int i = 0; i < fiber_count; i++)
        free(g_scheduler->fibers[i].stack);
    free(g_scheduler);
    g_scheduler = NULL;

    return had_deadlock;
}

void __pluto_test_run(long fn_ptr, long strategy, long seed, long iterations) {
    if (strategy == STRATEGY_SEQUENTIAL) {
        ((void(*)(void))fn_ptr)();
        return;
    }

    if (strategy == STRATEGY_EXHAUSTIVE) {
        // ── Exhaustive strategy: DFS over all interleavings with DPOR pruning ──
        int max_schedules = 10000;
        int max_depth = EXHST_MAX_DEPTH;
        char *env;
        env = getenv("PLUTO_MAX_SCHEDULES");
        if (env) max_schedules = (int)strtol(env, NULL, 0);
        env = getenv("PLUTO_MAX_DEPTH");
        if (env) {
            max_depth = (int)strtol(env, NULL, 0);
            if (max_depth > EXHST_MAX_DEPTH) max_depth = EXHST_MAX_DEPTH;
        }

        ExhaustiveState *es = (ExhaustiveState *)calloc(1, sizeof(ExhaustiveState));
        es->max_schedules = max_schedules;
        es->max_depth = max_depth;
        es->replay_len = 0;
        es->replay_next_choice = -1;  // first run: no forced choice, DFS picks first ready

        while (es->schedules_explored < es->max_schedules) {
            // Reset per-schedule state
            es->depth = 0;
            memset(es->fiber_channel_count, 0, sizeof(es->fiber_channel_count));
            g_exhaustive = es;

            int had_deadlock = test_run_single(fn_ptr, STRATEGY_EXHAUSTIVE, 0);

            es->fiber_count_snapshot = 0;  // infer from depth info
            g_exhaustive = NULL;

            // Collect failure info
            if (had_deadlock && es->failure_count < EXHST_MAX_FAILURES) {
                char msg[256];
                snprintf(msg, sizeof(msg), "deadlock in schedule %d (depth %d)",
                         es->schedules_explored, es->depth);
                es->failure_messages[es->failure_count++] = strdup(msg);
            }

            // Update DPOR dependency matrix from this schedule's channel accesses.
            // We need the fiber count — infer it from the scheduler that was just freed.
            // Since fibers are created incrementally (0..N-1), count from channel tracking.
            {
                int max_fiber = 0;
                for (int i = 0; i < MAX_FIBERS; i++) {
                    if (es->fiber_channel_count[i] > 0 && i + 1 > max_fiber)
                        max_fiber = i + 1;
                }
                // Also check the depth records for fibers that never touched channels
                for (int d = 0; d < es->depth; d++) {
                    for (int j = 0; j < es->ready_count[d]; j++) {
                        if (es->ready[d][j] + 1 > max_fiber)
                            max_fiber = es->ready[d][j] + 1;
                    }
                }
                if (max_fiber > 0)
                    exhaustive_update_dep_matrix(es, max_fiber);
            }

            es->schedules_explored++;

            // Find next unexplored schedule via backtracking
            if (!exhaustive_find_backtrack(es)) break;
        }

        // Report results
        fprintf(stderr, "  Exhaustive: %d schedule%s explored",
                es->schedules_explored, es->schedules_explored == 1 ? "" : "s");
        if (es->schedules_explored >= es->max_schedules) {
            fprintf(stderr, " (limit reached)");
        }
        fprintf(stderr, "\n");

        if (es->failure_count > 0) {
            fprintf(stderr, "  %d failure%s found:\n",
                    es->failure_count, es->failure_count == 1 ? "" : "s");
            for (int i = 0; i < es->failure_count; i++) {
                fprintf(stderr, "    - %s\n", es->failure_messages[i]);
                free(es->failure_messages[i]);
            }
            free(es);
            exit(1);
        }
        free(es);
        return;
    }

    // ── RoundRobin / Random strategies ──
    char *env_seed = getenv("PLUTO_TEST_SEED");
    if (env_seed) seed = (long)strtoull(env_seed, NULL, 0);
    char *env_iters = getenv("PLUTO_TEST_ITERATIONS");
    if (env_iters) iterations = (long)strtoull(env_iters, NULL, 0);

    int num_runs = (strategy == STRATEGY_RANDOM) ? (int)iterations : 1;
    if (num_runs < 1) num_runs = 1;

    for (int run = 0; run < num_runs; run++) {
        uint64_t run_seed = (uint64_t)seed + (uint64_t)run;
        int had_deadlock = test_run_single(fn_ptr, (Strategy)strategy, run_seed);
        if (had_deadlock) {
            fprintf(stderr, "  (seed: 0x%llx, iteration: %d)\n",
                    (unsigned long long)run_seed, run);
            exit(1);
        }
    }
}

// ── Test mode: task operations (fiber-aware) ────────────────────────────────

static long task_spawn_sequential(long closure_ptr) {
    // Phase A inline behavior (for sequential strategy or no scheduler)
    long *task = (long *)gc_alloc(56, GC_TAG_TASK, 3);
    task[0] = closure_ptr;
    task[1] = 0;  task[2] = 0;  task[3] = 0;
    task[4] = 0;  task[5] = 0;  task[6] = 0;

    long *prev_task = __pluto_current_task;
    void *prev_error = __pluto_current_error;
    __pluto_current_error = NULL;
    __pluto_current_task = task;

    long fn_ptr = *(long *)closure_ptr;
    long result = ((long(*)(long))fn_ptr)(closure_ptr);

    if (__pluto_current_error) {
        task[2] = (long)__pluto_current_error;
        __pluto_current_error = NULL;
    } else {
        task[1] = result;
    }
    task[3] = 1;

    if (task[5] && task[2]) {
        long *err_obj = (long *)task[2];
        char *msg_ptr = (char *)err_obj[0];
        if (msg_ptr) {
            long len = *(long *)msg_ptr;
            char *data = msg_ptr + 8;
            fprintf(stderr, "pluto: error in detached task: %.*s\n", (int)len, data);
        }
    }

    __pluto_current_task = prev_task;
    __pluto_current_error = prev_error;
    return (long)task;
}

static long task_spawn_fiber(long closure_ptr) {
    // Create a new fiber for the spawned task
    long *task = (long *)gc_alloc(56, GC_TAG_TASK, 3);
    task[0] = closure_ptr;
    task[1] = 0;  task[2] = 0;  task[3] = 0;
    task[4] = 0;  task[5] = 0;  task[6] = 0;

    int fid = g_scheduler->fiber_count;
    if (fid >= MAX_FIBERS) {
        fprintf(stderr, "pluto: too many fibers (max %d)\n", MAX_FIBERS);
        exit(1);
    }

    Fiber *f = &g_scheduler->fibers[fid];
    f->id = fid;
    f->state = FIBER_READY;
    f->stack = (char *)malloc(FIBER_STACK_SIZE);
    f->task = task;
    f->closure_ptr = closure_ptr;
    f->blocked_on = NULL;
    f->blocked_value = 0;
    f->saved_error = NULL;
    f->saved_current_task = task;  // fiber starts with its own task as current
    getcontext(&f->context);
    f->context.uc_stack.ss_sp = f->stack;
    f->context.uc_stack.ss_size = FIBER_STACK_SIZE;
    f->context.uc_link = &g_scheduler->scheduler_ctx;
    makecontext(&f->context, (void(*)(void))fiber_entry_fn, 1, fid);
    g_scheduler->fiber_count++;

    // Register with GC fiber stack scanner
    __pluto_gc_register_fiber_stack(f->stack, FIBER_STACK_SIZE);

    // Store fiber_id in task[4] for cross-referencing
    task[4] = (long)fid;

    return (long)task;
}

long __pluto_task_spawn(long closure_ptr) {
    if (!g_scheduler || g_scheduler->strategy == STRATEGY_SEQUENTIAL) {
        return task_spawn_sequential(closure_ptr);
    }
    return task_spawn_fiber(closure_ptr);
}

long __pluto_task_get(long task_ptr) {
    long *task = (long *)task_ptr;

    if (task[6] && !task[1] && !task[2]) {
        task_raise_cancelled();
        return 0;
    }

    if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
        // Fiber mode: if task not done, block and yield
        while (!task[3]) {
            Fiber *cur = &g_scheduler->fibers[g_scheduler->current_fiber];
            cur->state = FIBER_BLOCKED_TASK;
            cur->blocked_on = (void *)task;
            fiber_yield_to_scheduler();
            // Resumed — task should be done now (or we got woken spuriously)
        }
    }
    // Task is done (either was already done, or we waited)
    if (task[2]) {
        __pluto_current_error = (void *)task[2];
        return 0;
    }
    return task[1];
}

void __pluto_task_detach(long task_ptr) {
    long *task = (long *)task_ptr;
    task[5] = 1;
    if (task[3] && task[2]) {
        long *err_obj = (long *)task[2];
        char *msg_ptr = (char *)err_obj[0];
        if (msg_ptr) {
            long len = *(long *)msg_ptr;
            char *data = msg_ptr + 8;
            fprintf(stderr, "pluto: error in detached task: %.*s\n", (int)len, data);
        }
    }
}

void __pluto_task_cancel(long task_ptr) {
    long *task = (long *)task_ptr;
    task[6] = 1;
}

#else

// ── Production mode: pthread-based concurrency ──

typedef struct {
    pthread_mutex_t mutex;
    pthread_cond_t cond;
} TaskSync;

static void *__pluto_spawn_trampoline(void *arg) {
    long *task = (long *)arg;
    long closure_ptr = task[0];
    __pluto_current_error = NULL;  // clean TLS for new thread
    __pluto_current_task = task;   // set TLS for cancellation checks

    // Register this thread's stack with GC for root scanning
    int my_stack_slot = -1;
    {
        pthread_t self = pthread_self();
        void *stack_lo = NULL;
        void *stack_hi = NULL;
#ifdef __APPLE__
        stack_hi = pthread_get_stackaddr_np(self);
        size_t stack_sz = pthread_get_stacksize_np(self);
        stack_lo = (char *)stack_hi - stack_sz;
#else
        pthread_attr_t pattr;
        pthread_getattr_np(self, &pattr);
        size_t stack_sz;
        pthread_attr_getstack(&pattr, &stack_lo, &stack_sz);
        stack_hi = (char *)stack_lo + stack_sz;
        pthread_attr_destroy(&pattr);
#endif
        __pluto_gc_register_thread_stack(stack_lo, stack_hi);
    }

    long fn_ptr = *(long *)closure_ptr;
    long result = ((long(*)(long))fn_ptr)(closure_ptr);

    TaskSync *sync = (TaskSync *)task[4];
    pthread_mutex_lock(&sync->mutex);
    if (__pluto_current_error) {
        task[2] = (long)__pluto_current_error;
        __pluto_current_error = NULL;
    } else {
        task[1] = result;
    }
    task[3] = 1;  // done
    // If detached and errored, print to stderr
    if (task[5] && task[2]) {
        // Extract error message: error object has message field at slot 0
        long *err_obj = (long *)task[2];
        char *msg_ptr = (char *)err_obj[0];
        if (msg_ptr) {
            long len = *(long *)msg_ptr;
            char *data = msg_ptr + 8;
            fprintf(stderr, "pluto: error in detached task: %.*s\n", (int)len, data);
        }
    }
    pthread_cond_signal(&sync->cond);
    pthread_mutex_unlock(&sync->mutex);

    // Deregister thread stack from GC
    __pluto_gc_deregister_thread_stack();

    __pluto_current_task = NULL;
    __pluto_gc_task_end();
    return NULL;
}

long __pluto_task_spawn(long closure_ptr) {
    long *task = (long *)gc_alloc(56, GC_TAG_TASK, 3);
    task[0] = closure_ptr;
    task[1] = 0;  task[2] = 0;  task[3] = 0;
    task[5] = 0;  task[6] = 0;  // detached, cancelled

    TaskSync *sync = (TaskSync *)calloc(1, sizeof(TaskSync));
    pthread_mutex_init(&sync->mutex, NULL);
    pthread_cond_init(&sync->cond, NULL);
    task[4] = (long)sync;

    __pluto_gc_task_start();

    pthread_t tid;
    pthread_attr_t attr;
    pthread_attr_init(&attr);
    pthread_attr_setdetachstate(&attr, PTHREAD_CREATE_DETACHED);
    int ret = pthread_create(&tid, &attr, __pluto_spawn_trampoline, task);
    pthread_attr_destroy(&attr);
    if (ret != 0) {
        fprintf(stderr, "pluto: failed to create thread: %d\n", ret);
        exit(1);
    }
    return (long)task;
}

long __pluto_task_get(long task_ptr) {
    long *task = (long *)task_ptr;
    TaskSync *sync = (TaskSync *)task[4];

    pthread_mutex_lock(&sync->mutex);
    while (!task[3]) {
        // Use timed wait with short timeout to allow safepoint checks
        struct timespec ts;
        clock_gettime(CLOCK_REALTIME, &ts);
        ts.tv_nsec += 10000000;  // 10ms timeout
        if (ts.tv_nsec >= 1000000000) {
            ts.tv_sec += 1;
            ts.tv_nsec -= 1000000000;
        }
        pthread_cond_timedwait(&sync->cond, &sync->mutex, &ts);

        // Check safepoint while holding mutex (safe because safepoint doesn't need mutex)
        if (__pluto_gc_check_safepoint()) {
            pthread_mutex_unlock(&sync->mutex);
            __pluto_safepoint();
            pthread_mutex_lock(&sync->mutex);
        }
    }
    pthread_mutex_unlock(&sync->mutex);

    // If cancelled and no result, raise TaskCancelled
    if (task[6] && !task[1] && !task[2]) {
        task_raise_cancelled();
        return 0;
    }

    if (task[2]) {
        __pluto_current_error = (void *)task[2];
        return 0;
    }
    return task[1];
}

void __pluto_task_detach(long task_ptr) {
    long *task = (long *)task_ptr;
    TaskSync *sync = (TaskSync *)task[4];

    pthread_mutex_lock(&sync->mutex);
    task[5] = 1;  // mark as detached
    // If already done + errored, print to stderr now
    if (task[3] && task[2]) {
        long *err_obj = (long *)task[2];
        char *msg_ptr = (char *)err_obj[0];
        if (msg_ptr) {
            long len = *(long *)msg_ptr;
            char *data = msg_ptr + 8;
            fprintf(stderr, "pluto: error in detached task: %.*s\n", (int)len, data);
        }
    }
    pthread_mutex_unlock(&sync->mutex);
}

void __pluto_task_cancel(long task_ptr) {
    long *task = (long *)task_ptr;
    task[6] = 1;  // set cancelled flag
    // Wake the task thread if it's blocked on its own sync (for .get() waiters)
    TaskSync *sync = (TaskSync *)task[4];
    pthread_mutex_lock(&sync->mutex);
    pthread_cond_broadcast(&sync->cond);
    pthread_mutex_unlock(&sync->mutex);
}

#endif

// ── Deep Copy (for spawn isolation) ──────────────────────────────────────────

// Visited table for cycle detection during deep copy
typedef struct {
    void **originals;
    void **copies;
    size_t count;
    size_t cap;
} DeepCopyVisited;

static void dc_visited_init(DeepCopyVisited *v) {
    v->count = 0;
    v->cap = 16;
    v->originals = (void **)malloc(v->cap * sizeof(void *));
    v->copies    = (void **)malloc(v->cap * sizeof(void *));
}

static void dc_visited_free(DeepCopyVisited *v) {
    free(v->originals);
    free(v->copies);
}

static void *dc_visited_lookup(DeepCopyVisited *v, void *original) {
    for (size_t i = 0; i < v->count; i++) {
        if (v->originals[i] == original) return v->copies[i];
    }
    return NULL;
}

static void dc_visited_insert(DeepCopyVisited *v, void *original, void *copy) {
    if (v->count >= v->cap) {
        v->cap *= 2;
        v->originals = (void **)realloc(v->originals, v->cap * sizeof(void *));
        v->copies    = (void **)realloc(v->copies,    v->cap * sizeof(void *));
    }
    v->originals[v->count] = original;
    v->copies[v->count]    = copy;
    v->count++;
}

// Check if a value is a pointer to the start of a GC object's user data.
// Linear scan of gc_head — acceptable because spawn is not a hot path.
static GCHeader *dc_find_gc_object(void *candidate) {
    GCHeader *h = __pluto_gc_get_head();
    while (h) {
        void *user = (char *)h + sizeof(GCHeader);
        if (user == candidate) return h;
        h = h->next;
    }
    return NULL;
}

static long dc_deep_copy_impl(long ptr, DeepCopyVisited *visited);

// Recursively deep-copy a slot value if it's a GC pointer
static long dc_copy_slot(long slot_val, DeepCopyVisited *visited) {
    if (slot_val == 0) return 0;
    GCHeader *h = dc_find_gc_object((void *)slot_val);
    if (!h) return slot_val;  // Not a GC pointer — primitive value
    return dc_deep_copy_impl(slot_val, visited);
}

static long dc_deep_copy_impl(long ptr, DeepCopyVisited *visited) {
    if (ptr == 0) return 0;

    void *orig = (void *)ptr;
    GCHeader *h = dc_find_gc_object(orig);
    if (!h) return ptr;  // Not a GC object — return as-is

    // Check visited (cycle detection)
    void *existing = dc_visited_lookup(visited, orig);
    if (existing) return (long)existing;

    switch (h->type_tag) {
    case GC_TAG_STRING:
        // Strings are immutable — no copy needed
        return ptr;

    case GC_TAG_TASK:
    case GC_TAG_CHANNEL:
        // Tasks and channels are shared by reference
        return ptr;

    case GC_TAG_OBJECT: {
        // Classes, enums, closures, errors
        // Layout: field_count * 8 bytes of slots
        uint16_t fc = h->field_count;
        void *copy = gc_alloc(h->size, GC_TAG_OBJECT, fc);
        dc_visited_insert(visited, orig, copy);
        memcpy(copy, orig, h->size);
        // Recursively deep-copy slots that are GC pointers
        long *src_slots = (long *)orig;
        long *dst_slots = (long *)copy;
        for (uint16_t i = 0; i < fc; i++) {
            dst_slots[i] = dc_copy_slot(src_slots[i], visited);
        }
        return (long)copy;
    }

    case GC_TAG_ARRAY: {
        // Handle: [len][cap][data_ptr]
        long *src = (long *)orig;
        long len = src[0];
        long cap = src[1];
        long *src_data = (long *)src[2];

        long *copy = (long *)gc_alloc(24, GC_TAG_ARRAY, 3);
        dc_visited_insert(visited, orig, copy);
        copy[0] = len;
        copy[1] = cap;
        // Allocate new data buffer (raw malloc, like __pluto_array_new)
        long *new_data = (long *)calloc((size_t)cap, sizeof(long));
        copy[2] = (long)new_data;
        // Deep-copy each element
        for (long i = 0; i < len; i++) {
            new_data[i] = dc_copy_slot(src_data[i], visited);
        }
        return (long)copy;
    }

    case GC_TAG_BYTES: {
        // Handle: [len][cap][data_ptr]
        long *src = (long *)orig;
        long len = src[0];
        long cap = src[1];
        unsigned char *src_data = (unsigned char *)src[2];

        long *copy = (long *)gc_alloc(24, GC_TAG_BYTES, 3);
        dc_visited_insert(visited, orig, copy);
        copy[0] = len;
        copy[1] = cap;
        unsigned char *new_data = (unsigned char *)calloc((size_t)cap, 1);
        memcpy(new_data, src_data, (size_t)len);
        copy[2] = (long)new_data;
        return (long)copy;
    }

    case GC_TAG_TRAIT: {
        // Handle: [data_ptr][vtable_ptr]
        long *src = (long *)orig;
        long *copy = (long *)gc_alloc(16, GC_TAG_TRAIT, 2);
        dc_visited_insert(visited, orig, copy);
        copy[0] = dc_copy_slot(src[0], visited);  // deep-copy underlying data
        copy[1] = src[1];  // vtable pointer stays the same
        return (long)copy;
    }

    case GC_TAG_MAP: {
        // Handle: [count][cap][keys_ptr][vals_ptr][meta_ptr]
        long *src = (long *)orig;
        long count = src[0];
        long cap = src[1];
        long *src_keys = (long *)src[2];
        long *src_vals = (long *)src[3];
        unsigned char *src_meta = (unsigned char *)src[4];

        long *copy = (long *)gc_alloc(40, GC_TAG_MAP, 5);
        dc_visited_insert(visited, orig, copy);
        copy[0] = count;
        copy[1] = cap;

        long *new_keys = (long *)calloc((size_t)cap, sizeof(long));
        long *new_vals = (long *)calloc((size_t)cap, sizeof(long));
        unsigned char *new_meta = (unsigned char *)calloc((size_t)cap, 1);
        memcpy(new_meta, src_meta, (size_t)cap);
        copy[2] = (long)new_keys;
        copy[3] = (long)new_vals;
        copy[4] = (long)new_meta;

        for (long i = 0; i < cap; i++) {
            if (src_meta[i] >= 0x80) {
                new_keys[i] = dc_copy_slot(src_keys[i], visited);
                new_vals[i] = dc_copy_slot(src_vals[i], visited);
            }
        }
        return (long)copy;
    }

    case GC_TAG_SET: {
        // Handle: [count][cap][keys_ptr][meta_ptr]
        long *src = (long *)orig;
        long count = src[0];
        long cap = src[1];
        long *src_keys = (long *)src[2];
        unsigned char *src_meta = (unsigned char *)src[3];

        long *copy = (long *)gc_alloc(32, GC_TAG_SET, 4);
        dc_visited_insert(visited, orig, copy);
        copy[0] = count;
        copy[1] = cap;

        long *new_keys = (long *)calloc((size_t)cap, sizeof(long));
        unsigned char *new_meta = (unsigned char *)calloc((size_t)cap, 1);
        memcpy(new_meta, src_meta, (size_t)cap);
        copy[2] = (long)new_keys;
        copy[3] = (long)new_meta;

        for (long i = 0; i < cap; i++) {
            if (src_meta[i] >= 0x80) {
                new_keys[i] = dc_copy_slot(src_keys[i], visited);
            }
        }
        return (long)copy;
    }

    default:
        // Unknown tag — return as-is
        return ptr;
    }
}

long __pluto_deep_copy(long ptr) {
    DeepCopyVisited visited;
    dc_visited_init(&visited);
    long result = dc_deep_copy_impl(ptr, &visited);
    dc_visited_free(&visited);
    return result;
}

// ── Channels ────────────────────────────────────────────────────────────────

// Channel handle layout (56 bytes, 7 slots):
//   [0] sync_ptr   (raw malloc'd ChannelSync)
//   [1] buf_ptr    (raw malloc'd circular buffer of i64)
//   [2] capacity   (int, always >= 1)
//   [3] count      (int, items in buffer)
//   [4] head       (int, read position)
//   [5] tail       (int, write position)
//   [6] closed     (int, 0 or 1)

static void chan_raise_error(const char *msg) {
    void *msg_str = __pluto_string_new((char *)msg, (long)strlen(msg));
    void *err_obj = __pluto_alloc(8);  // 1 field: message
    *(long *)err_obj = (long)msg_str;
    __pluto_raise_error(err_obj);
}

#ifdef PLUTO_TEST_MODE

// ── Test mode: channel operations (fiber-aware) ──

long __pluto_chan_create(long capacity) {
    long actual_cap = capacity > 0 ? capacity : 1;
    long *ch = (long *)gc_alloc(64, GC_TAG_CHANNEL, 0);
    ch[0] = 0;  // no sync needed in test mode
    long *buf = (long *)calloc((size_t)actual_cap, sizeof(long));
    ch[1] = (long)buf;
    ch[2] = actual_cap;
    ch[3] = 0;  // count
    ch[4] = 0;  // head
    ch[5] = 0;  // tail
    ch[6] = 0;  // closed
    ch[7] = 1;  // sender_count
    return (long)ch;
}

long __pluto_chan_send(long handle, long value) {
    long *ch = (long *)handle;

    if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
        // Record channel access for DPOR dependency tracking
        exhaustive_record_channel(g_scheduler->current_fiber, (void *)ch);

        // Fiber mode: yield when buffer is full
        while (1) {
            if (ch[6]) {
                chan_raise_error("channel closed");
                return 0;
            }
            if (ch[3] < ch[2]) {
                // Space available — push value
                long *buf = (long *)ch[1];
                buf[ch[5]] = value;
                ch[5] = (ch[5] + 1) % ch[2];
                ch[3]++;
                // Wake any fibers waiting to recv on this channel
                wake_fibers_blocked_on_chan(ch);
                wake_select_fibers_for_chan(ch);
                return value;
            }
            // Buffer full — yield
            Fiber *cur = &g_scheduler->fibers[g_scheduler->current_fiber];
            cur->state = FIBER_BLOCKED_CHAN_SEND;
            cur->blocked_on = (void *)ch;
            cur->blocked_value = value;
            fiber_yield_to_scheduler();
            // Resumed — retry
        }
    }

    // Sequential mode
    if (ch[6]) {
        chan_raise_error("channel closed");
        return 0;
    }
    if (ch[3] == ch[2]) {
        fprintf(stderr, "pluto: deadlock detected — channel send on full buffer in sequential test mode\n");
        exit(1);
    }
    long *buf = (long *)ch[1];
    buf[ch[5]] = value;
    ch[5] = (ch[5] + 1) % ch[2];
    ch[3]++;
    return value;
}

long __pluto_chan_recv(long handle) {
    long *ch = (long *)handle;

    if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
        // Record channel access for DPOR dependency tracking
        exhaustive_record_channel(g_scheduler->current_fiber, (void *)ch);

        // Fiber mode: yield when buffer is empty
        while (1) {
            if (ch[3] > 0) {
                // Data available — pop value
                long *buf = (long *)ch[1];
                long val = buf[ch[4]];
                ch[4] = (ch[4] + 1) % ch[2];
                ch[3]--;
                // Wake any fibers waiting to send on this channel
                wake_fibers_blocked_on_chan(ch);
                wake_select_fibers_for_chan(ch);
                return val;
            }
            if (ch[6]) {
                chan_raise_error("channel closed");
                return 0;
            }
            // Buffer empty — yield
            Fiber *cur = &g_scheduler->fibers[g_scheduler->current_fiber];
            cur->state = FIBER_BLOCKED_CHAN_RECV;
            cur->blocked_on = (void *)ch;
            fiber_yield_to_scheduler();
            // Resumed — retry
        }
    }

    // Sequential mode
    if (ch[3] == 0 && ch[6]) {
        chan_raise_error("channel closed");
        return 0;
    }
    if (ch[3] == 0) {
        fprintf(stderr, "pluto: deadlock detected — channel recv on empty buffer in sequential test mode\n");
        exit(1);
    }
    long *buf = (long *)ch[1];
    long val = buf[ch[4]];
    ch[4] = (ch[4] + 1) % ch[2];
    ch[3]--;
    return val;
}

long __pluto_chan_try_send(long handle, long value) {
    long *ch = (long *)handle;
    if (ch[6]) {
        chan_raise_error("channel closed");
        return 0;
    }
    if (ch[3] == ch[2]) {
        chan_raise_error("channel full");
        return 0;
    }
    long *buf = (long *)ch[1];
    buf[ch[5]] = value;
    ch[5] = (ch[5] + 1) % ch[2];
    ch[3]++;
    if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
        wake_fibers_blocked_on_chan(ch);
        wake_select_fibers_for_chan(ch);
    }
    return value;
}

long __pluto_chan_try_recv(long handle) {
    long *ch = (long *)handle;
    if (ch[3] == 0 && ch[6]) {
        chan_raise_error("channel closed");
        return 0;
    }
    if (ch[3] == 0) {
        chan_raise_error("channel empty");
        return 0;
    }
    long *buf = (long *)ch[1];
    long val = buf[ch[4]];
    ch[4] = (ch[4] + 1) % ch[2];
    ch[3]--;
    if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
        wake_fibers_blocked_on_chan(ch);
        wake_select_fibers_for_chan(ch);
    }
    return val;
}

void __pluto_chan_close(long handle) {
    long *ch = (long *)handle;
    ch[6] = 1;
    if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
        // Wake ALL fibers blocked on this channel (both send and recv)
        wake_fibers_blocked_on_chan(ch);
        wake_select_fibers_for_chan(ch);
    }
}

void __pluto_chan_sender_inc(long handle) {
    long *ch = (long *)handle;
    if (!ch) return;
    ch[7]++;
}

void __pluto_chan_sender_dec(long handle) {
    long *ch = (long *)handle;
    if (!ch) return;
    long old = ch[7];
    ch[7]--;
    if (old <= 0) {
        ch[7]++;
        return;
    }
    if (old == 1) {
        __pluto_chan_close(handle);
    }
}

#else

// ── Production mode: mutex-protected channel operations ──

long __pluto_chan_create(long capacity) {
    long actual_cap = capacity > 0 ? capacity : 1;
    // field_count=0: slots 0-1 are raw malloc ptrs, 2-7 are ints; GC_TAG_CHANNEL traces buffer
    long *ch = (long *)gc_alloc(64, GC_TAG_CHANNEL, 0);

    ChannelSync *sync = (ChannelSync *)calloc(1, sizeof(ChannelSync));
    pthread_mutex_init(&sync->mutex, NULL);
    pthread_cond_init(&sync->not_empty, NULL);
    pthread_cond_init(&sync->not_full, NULL);

    long *buf = (long *)calloc((size_t)actual_cap, sizeof(long));

    ch[0] = (long)sync;
    ch[1] = (long)buf;
    ch[2] = actual_cap;
    ch[3] = 0;  // count
    ch[4] = 0;  // head
    ch[5] = 0;  // tail
    ch[6] = 0;  // closed
    ch[7] = 1;  // sender_count (starts at 1 for the initial LetChan sender)
    return (long)ch;
}

long __pluto_chan_send(long handle, long value) {
    long *ch = (long *)handle;
    ChannelSync *sync = (ChannelSync *)ch[0];

    pthread_mutex_lock(&sync->mutex);
    while (ch[3] == ch[2] && !ch[6]) {
        pthread_cond_wait(&sync->not_full, &sync->mutex);
        // Check for task cancellation after waking from condvar
        if (__pluto_current_task && __pluto_current_task[6]) {
            pthread_mutex_unlock(&sync->mutex);
            task_raise_cancelled();
            return 0;
        }
    }
    if (ch[6]) {
        pthread_mutex_unlock(&sync->mutex);
        chan_raise_error("channel closed");
        return 0;
    }
    long *buf = (long *)ch[1];
    buf[ch[5]] = value;
    ch[5] = (ch[5] + 1) % ch[2];
    ch[3]++;
    pthread_cond_signal(&sync->not_empty);
    pthread_mutex_unlock(&sync->mutex);
    return value;
}

long __pluto_chan_recv(long handle) {
    long *ch = (long *)handle;
    ChannelSync *sync = (ChannelSync *)ch[0];

    pthread_mutex_lock(&sync->mutex);
    while (ch[3] == 0 && !ch[6]) {
        pthread_cond_wait(&sync->not_empty, &sync->mutex);
        // Check for task cancellation after waking from condvar
        if (__pluto_current_task && __pluto_current_task[6]) {
            pthread_mutex_unlock(&sync->mutex);
            task_raise_cancelled();
            return 0;
        }
    }
    if (ch[3] == 0 && ch[6]) {
        pthread_mutex_unlock(&sync->mutex);
        chan_raise_error("channel closed");
        return 0;
    }
    long *buf = (long *)ch[1];
    long val = buf[ch[4]];
    ch[4] = (ch[4] + 1) % ch[2];
    ch[3]--;
    pthread_cond_signal(&sync->not_full);
    pthread_mutex_unlock(&sync->mutex);
    return val;
}

long __pluto_chan_try_send(long handle, long value) {
    long *ch = (long *)handle;
    ChannelSync *sync = (ChannelSync *)ch[0];

    pthread_mutex_lock(&sync->mutex);
    if (ch[6]) {
        pthread_mutex_unlock(&sync->mutex);
        chan_raise_error("channel closed");
        return 0;
    }
    if (ch[3] == ch[2]) {
        pthread_mutex_unlock(&sync->mutex);
        chan_raise_error("channel full");
        return 0;
    }
    long *buf = (long *)ch[1];
    buf[ch[5]] = value;
    ch[5] = (ch[5] + 1) % ch[2];
    ch[3]++;
    pthread_cond_signal(&sync->not_empty);
    pthread_mutex_unlock(&sync->mutex);
    return value;
}

long __pluto_chan_try_recv(long handle) {
    long *ch = (long *)handle;
    ChannelSync *sync = (ChannelSync *)ch[0];

    pthread_mutex_lock(&sync->mutex);
    if (ch[3] == 0 && ch[6]) {
        pthread_mutex_unlock(&sync->mutex);
        chan_raise_error("channel closed");
        return 0;
    }
    if (ch[3] == 0) {
        pthread_mutex_unlock(&sync->mutex);
        chan_raise_error("channel empty");
        return 0;
    }
    long *buf = (long *)ch[1];
    long val = buf[ch[4]];
    ch[4] = (ch[4] + 1) % ch[2];
    ch[3]--;
    pthread_cond_signal(&sync->not_full);
    pthread_mutex_unlock(&sync->mutex);
    return val;
}

void __pluto_chan_close(long handle) {
    long *ch = (long *)handle;
    ChannelSync *sync = (ChannelSync *)ch[0];

    pthread_mutex_lock(&sync->mutex);
    ch[6] = 1;
    pthread_cond_broadcast(&sync->not_empty);
    pthread_cond_broadcast(&sync->not_full);
    pthread_mutex_unlock(&sync->mutex);
}

void __pluto_chan_sender_inc(long handle) {
    long *ch = (long *)handle;
    if (!ch) return;  // null guard for pre-declared vars
    __atomic_fetch_add(&ch[7], 1, __ATOMIC_SEQ_CST);
}

void __pluto_chan_sender_dec(long handle) {
    long *ch = (long *)handle;
    if (!ch) return;  // null guard for pre-declared vars
    long old = __atomic_fetch_sub(&ch[7], 1, __ATOMIC_SEQ_CST);
    if (old <= 0) {
        // Underflow guard: undo dec, fail safe
        __atomic_fetch_add(&ch[7], 1, __ATOMIC_SEQ_CST);
        return;
    }
    if (old == 1) {
        __pluto_chan_close(handle);  // last sender -> auto-close
    }
}

#endif

// ── Select (channel multiplexing) ──────────────────────────

/*
 * __pluto_select(buffer, count, has_default) -> case index
 *
 * Buffer layout (3 * count i64 slots):
 *   buffer[0..count)          = channel handles
 *   buffer[count..2*count)    = ops (0 = recv, 1 = send)
 *   buffer[2*count..3*count)  = values (send values in, recv values out)
 *
 * Returns:
 *   >= 0  : index of the case that completed
 *   -1    : default case (only when has_default)
 *   -2    : all channels closed (error raised via TLS)
 */
#ifdef PLUTO_TEST_MODE

// ── Test mode: select (fiber-aware) ──

static long select_try_arms(long *handles, long *ops, long *values, int n, int *indices) {
    int all_closed = 1;
    for (int si = 0; si < n; si++) {
        int i = indices[si];
        long *ch = (long *)handles[i];
        if (ops[i] == 0) {
            /* recv */
            if (ch[3] > 0) {
                long *cbuf = (long *)ch[1];
                long val = cbuf[ch[4]];
                ch[4] = (ch[4] + 1) % ch[2];
                ch[3]--;
                values[i] = val;
                if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
                    wake_fibers_blocked_on_chan(ch);
                    wake_select_fibers_for_chan(ch);
                }
                return (long)i;
            }
            if (!ch[6]) all_closed = 0;
        } else {
            /* send */
            if (!ch[6] && ch[3] < ch[2]) {
                long *cbuf = (long *)ch[1];
                cbuf[ch[5]] = values[i];
                ch[5] = (ch[5] + 1) % ch[2];
                ch[3]++;
                if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
                    wake_fibers_blocked_on_chan(ch);
                    wake_select_fibers_for_chan(ch);
                }
                return (long)i;
            }
            if (!ch[6]) all_closed = 0;
        }
    }
    if (all_closed) return -2;
    return -3;  // no ready arm, not all closed
}

long __pluto_select(long buffer_ptr, long count, long has_default) {
    long *buf = (long *)buffer_ptr;
    long *handles = &buf[0];
    long *ops     = &buf[count];
    long *values  = &buf[2 * count];

    /* Fisher-Yates shuffle for fairness */
    int indices[64];
    int n = (int)count;
    if (n > 64) n = 64;
    for (int i = 0; i < n; i++) indices[i] = i;
    unsigned long seed = (unsigned long)buffer_ptr ^ (unsigned long)__pluto_time_ns();
    for (int i = n - 1; i > 0; i--) {
        seed = seed * 6364136223846793005ULL + 1442695040888963407ULL;
        int j = (int)((seed >> 33) % (unsigned long)(i + 1));
        int tmp = indices[i]; indices[i] = indices[j]; indices[j] = tmp;
    }

    if (g_scheduler && g_scheduler->strategy != STRATEGY_SEQUENTIAL) {
        // Record all channels in this select for DPOR dependency tracking
        for (int si = 0; si < n; si++) {
            exhaustive_record_channel(g_scheduler->current_fiber, (void *)handles[si]);
        }

        // Fiber mode: loop with yield
        while (1) {
            long result = select_try_arms(handles, ops, values, n, indices);
            if (result >= 0) return result;
            if (has_default) return -1;
            if (result == -2) {
                chan_raise_error("channel closed");
                return -2;
            }
            // Block and yield
            Fiber *cur = &g_scheduler->fibers[g_scheduler->current_fiber];
            cur->state = FIBER_BLOCKED_SELECT;
            cur->blocked_on = (void *)buf;
            fiber_yield_to_scheduler();
            // Resumed — retry all arms
        }
    }

    // Sequential mode: single pass
    long result = select_try_arms(handles, ops, values, n, indices);
    if (result >= 0) return result;
    if (has_default) return -1;
    if (result == -2) {
        chan_raise_error("channel closed");
        return -2;
    }
    fprintf(stderr, "pluto: deadlock detected — select with no ready channels in sequential test mode\n");
    exit(1);
}

#else

// ── Production mode: spin-poll select ──

long __pluto_select(long buffer_ptr, long count, long has_default) {
    long *buf = (long *)buffer_ptr;
    long *handles = &buf[0];
    long *ops     = &buf[count];
    long *values  = &buf[2 * count];

    /* Fisher-Yates shuffle for fairness */
    int indices[64]; /* max 64 arms should be plenty */
    int n = (int)count;
    if (n > 64) n = 64;
    for (int i = 0; i < n; i++) indices[i] = i;
    /* simple LCG seeded from time + address entropy */
    unsigned long seed = (unsigned long)buffer_ptr ^ (unsigned long)__pluto_time_ns();

    for (int i = n - 1; i > 0; i--) {
        seed = seed * 6364136223846793005ULL + 1442695040888963407ULL;
        int j = (int)((seed >> 33) % (unsigned long)(i + 1));
        int tmp = indices[i]; indices[i] = indices[j]; indices[j] = tmp;
    }

    /* Spin-poll loop */
    long spin_us = 100;  /* start at 100 microseconds */
    for (;;) {
        int all_closed = 1;

        for (int si = 0; si < n; si++) {
            int i = indices[si];
            long *ch = (long *)handles[i];
            ChannelSync *sync = (ChannelSync *)ch[0];

            pthread_mutex_lock(&sync->mutex);

            if (ops[i] == 0) {
                /* recv */
                if (ch[3] > 0) {
                    /* data available */
                    long *cbuf = (long *)ch[1];
                    long val = cbuf[ch[4]];
                    ch[4] = (ch[4] + 1) % ch[2];
                    ch[3]--;
                    pthread_cond_signal(&sync->not_full);
                    pthread_mutex_unlock(&sync->mutex);
                    values[i] = val;
                    return (long)i;
                }
                if (!ch[6]) {
                    all_closed = 0;
                }
            } else {
                /* send */
                if (!ch[6] && ch[3] < ch[2]) {
                    /* space available */
                    long *cbuf = (long *)ch[1];
                    cbuf[ch[5]] = values[i];
                    ch[5] = (ch[5] + 1) % ch[2];
                    ch[3]++;
                    pthread_cond_signal(&sync->not_empty);
                    pthread_mutex_unlock(&sync->mutex);
                    return (long)i;
                }
                if (!ch[6]) {
                    all_closed = 0;
                }
            }

            pthread_mutex_unlock(&sync->mutex);
        }

        if (has_default) {
            return -1;
        }

        if (all_closed) {
            /* Raise ChannelClosed error */
            chan_raise_error("channel closed");
            return -2;
        }

        /* Adaptive sleep: 100us -> 200us -> ... -> 1ms max */
        usleep((useconds_t)spin_us);
        if (spin_us < 1000) spin_us = spin_us * 2;
        if (spin_us > 1000) spin_us = 1000;
    }
}

#endif

// ── Contracts ──────────────────────────────────────────────

void __pluto_invariant_violation(long class_name, long invariant_desc) {
    // class_name and invariant_desc are Pluto strings (length-prefixed)
    long *name_ptr = (long *)class_name;
    long name_len = name_ptr[0];
    char *name_data = (char *)&name_ptr[1];

    long *desc_ptr = (long *)invariant_desc;
    long desc_len = desc_ptr[0];
    char *desc_data = (char *)&desc_ptr[1];

    fprintf(stderr, "invariant violation on %.*s: %.*s\n",
            (int)name_len, name_data, (int)desc_len, desc_data);
    exit(1);
}

void __pluto_requires_violation(long fn_name, long contract_desc) {
    long *name_ptr = (long *)fn_name;
    long name_len = name_ptr[0];
    char *name_data = (char *)&name_ptr[1];

    long *desc_ptr = (long *)contract_desc;
    long desc_len = desc_ptr[0];
    char *desc_data = (char *)&desc_ptr[1];

    fprintf(stderr, "requires violation in %.*s: %.*s\n",
            (int)name_len, name_data, (int)desc_len, desc_data);
    exit(1);
}

void __pluto_ensures_violation(long fn_name, long contract_desc) {
    long *name_ptr = (long *)fn_name;
    long name_len = name_ptr[0];
    char *name_data = (char *)&name_ptr[1];

    long *desc_ptr = (long *)contract_desc;
    long desc_len = desc_ptr[0];
    char *desc_data = (char *)&desc_ptr[1];

    fprintf(stderr, "ensures violation in %.*s: %.*s\n",
            (int)name_len, name_data, (int)desc_len, desc_data);
    exit(1);
}

// ── Rwlock synchronization ─────────────────────────────────────────────────

#ifndef PLUTO_TEST_MODE
long __pluto_rwlock_init(void) {
    pthread_rwlock_t *lock = (pthread_rwlock_t *)malloc(sizeof(pthread_rwlock_t));
    pthread_rwlock_init(lock, NULL);
    return (long)lock;
}

void __pluto_rwlock_rdlock(long lock_ptr) {
    pthread_rwlock_rdlock((pthread_rwlock_t *)lock_ptr);
}

void __pluto_rwlock_wrlock(long lock_ptr) {
    pthread_rwlock_wrlock((pthread_rwlock_t *)lock_ptr);
}

void __pluto_rwlock_unlock(long lock_ptr) {
    pthread_rwlock_unlock((pthread_rwlock_t *)lock_ptr);
}
#endif

// ── Logging ────────────────────────────────────────────────────────────────

static int __pluto_global_log_level = 1;  // Default to INFO (1)

long __pluto_log_get_level(void) {
    return __pluto_global_log_level;
}

void __pluto_log_set_level(long level) {
    __pluto_global_log_level = (int)level;
}

void __pluto_log_write(void *level_str, long timestamp, void *message) {
    const char *level = (const char *)level_str + 8;
    const char *msg = (const char *)message + 8;
    fprintf(stderr, "[%s] %ld %s\n", level, timestamp, msg);
    fflush(stderr);
}

void __pluto_log_write_structured(void *level_str, long timestamp, void *message, long fields_ptr) {
    const char *level = (const char *)level_str + 8;
    const char *msg = (const char *)message + 8;
    fprintf(stderr, "[%s] %ld %s", level, timestamp, msg);
    
    long *arr_header = (long *)fields_ptr;
    long len = arr_header[0];
    long *data = (long *)arr_header[2];
    
    for (long i = 0; i < len; i++) {
        long *field_obj = (long *)data[i];
        void *key_ptr = (void *)field_obj[1];
        void *value_ptr = (void *)field_obj[2];
        const char *key = (const char *)key_ptr + 8;
        const char *value = (const char *)value_ptr + 8;
        fprintf(stderr, " %s=%s", key, value);
    }
    fprintf(stderr, "\n");
    fflush(stderr);
}

// ── Environment Variables ──────────────────────────────────────────────────

extern char **environ;

static void *__pluto_make_string(const char *c_str) {
    if (!c_str) {
        void *header = gc_alloc(8 + 1, GC_TAG_STRING, 0);
        *(long *)header = 0;
        ((char *)header)[8] = '\0';
        return header;
    }
    long len = (long)strlen(c_str);
    void *header = gc_alloc(8 + len + 1, GC_TAG_STRING, 0);
    *(long *)header = len;
    memcpy((char *)header + 8, c_str, len);
    ((char *)header)[8 + len] = '\0';
    return header;
}

void *__pluto_env_get(void *name_ptr) {
    long *name_header = (long *)name_ptr;
    long name_len = name_header[0];
    char *name_data = (char *)&name_header[1];

    char name_buf[1024];
    if (name_len >= 1024) {
        return __pluto_make_string("");
    }
    memcpy(name_buf, name_data, name_len);
    name_buf[name_len] = '\0';

    const char *val = getenv(name_buf);
    return __pluto_make_string(val);
}

void *__pluto_env_get_or(void *name_ptr, void *default_ptr) {
    long *name_header = (long *)name_ptr;
    long name_len = name_header[0];
    char *name_data = (char *)&name_header[1];

    char name_buf[1024];
    if (name_len >= 1024) {
        return default_ptr;
    }
    memcpy(name_buf, name_data, name_len);
    name_buf[name_len] = '\0';

    const char *val = getenv(name_buf);
    if (!val) {
        return default_ptr;
    }
    return __pluto_make_string(val);
}

void __pluto_env_set(void *name_ptr, void *value_ptr) {
    long *name_header = (long *)name_ptr;
    long name_len = name_header[0];
    char *name_data = (char *)&name_header[1];

    long *val_header = (long *)value_ptr;
    long val_len = val_header[0];
    char *val_data = (char *)&val_header[1];

    char name_buf[1024];
    char val_buf[4096];

    if (name_len >= 1024 || val_len >= 4096) {
        return;
    }

    memcpy(name_buf, name_data, name_len);
    name_buf[name_len] = '\0';
    memcpy(val_buf, val_data, val_len);
    val_buf[val_len] = '\0';

    setenv(name_buf, val_buf, 1);
}

long __pluto_env_exists(void *name_ptr) {
    long *name_header = (long *)name_ptr;
    long name_len = name_header[0];
    char *name_data = (char *)&name_header[1];

    char name_buf[1024];
    if (name_len >= 1024) {
        return 0;
    }
    memcpy(name_buf, name_data, name_len);
    name_buf[name_len] = '\0';

    return getenv(name_buf) != NULL ? 1 : 0;
}

void *__pluto_env_list_names() {
    // Count environment variables
    int count = 0;
    for (int i = 0; environ[i] != NULL; i++) {
        count++;
    }

    // Create array of strings
    void *arr = __pluto_array_new(count);

    for (int i = 0; i < count; i++) {
        char *env_str = environ[i];
        // Find the '=' separator
        char *eq = strchr(env_str, '=');
        if (!eq) {
            __pluto_array_push(arr, (long)__pluto_make_string(""));
            continue;
        }

        // Extract variable name (everything before '=')
        int name_len = (int)(eq - env_str);
        char name_buf[1024];
        if (name_len >= 1024) {
            __pluto_array_push(arr, (long)__pluto_make_string(""));
            continue;
        }

        memcpy(name_buf, env_str, name_len);
        name_buf[name_len] = '\0';

        __pluto_array_push(arr, (long)__pluto_make_string(name_buf));
    }

    return arr;
}

long __pluto_env_clear(void *name_ptr) {
    long *name_header = (long *)name_ptr;
    long name_len = name_header[0];
    char *name_data = (char *)&name_header[1];

    char name_buf[1024];
    if (name_len >= 1024) {
        return 0;
    }
    memcpy(name_buf, name_data, name_len);
    name_buf[name_len] = '\0';

    // unsetenv returns 0 on success, -1 on error
    return unsetenv(name_buf) == 0 ? 1 : 0;
}

// HTTP POST for RPC calls
// For MVP/testing: returns a dummy JSON response
// TODO: Implement actual HTTP client with libcurl or sockets
void *__pluto_http_post(void *url_ptr, void *body_ptr, long timeout_ms) {
    // Extract URL string
    long *url_header = (long *)url_ptr;
    long url_len = url_header[0];
    char *url_data = (char *)&url_header[1];

    // Extract body string
    long *body_header = (long *)body_ptr;
    long body_len = body_header[0];
    char *body_data = (char *)&body_header[1];

    // For MVP: just return a dummy JSON response for testing
    // In test mode, this will be intercepted before actual HTTP happens
    // Return quoted value so both int and string extraction can work
    char *response = "{\"status\":\"ok\",\"result\":\"42\"}";
    return __pluto_make_string(response);
}

// ── RPC Response Parsing ───────────────────────────────────────────────────────
// For MVP: Simple JSON parsing to extract "result" field from response
// Assumes format: {"status":"ok","result":VALUE}

// Extract int from JSON response
long __pluto_rpc_extract_int(void *response_ptr) {
    long *header = (long *)response_ptr;
    long len = header[0];
    const char *data = (const char *)&header[1];

    // Find "result": in the JSON
    const char *result_key = "\"result\":";
    const char *pos = strstr(data, result_key);
    if (!pos) {
        fprintf(stderr, "RPC Error: could not find 'result' in JSON response\n");
        exit(1);
    }

    // Skip the "result": part
    pos += strlen(result_key);

    // Skip opening quote if present (handles both "42" and 42)
    if (*pos == '"') {
        pos++;
    }

    // Parse the integer value
    long value = strtol(pos, NULL, 10);
    return value;
}

// Extract float from JSON response
double __pluto_rpc_extract_float(void *response_ptr) {
    long *header = (long *)response_ptr;
    long len = header[0];
    const char *data = (const char *)&header[1];

    const char *result_key = "\"result\":";
    const char *pos = strstr(data, result_key);
    if (!pos) {
        fprintf(stderr, "RPC Error: could not find 'result' in JSON response\n");
        exit(1);
    }

    pos += strlen(result_key);

    // Skip opening quote if present
    if (*pos == '"') {
        pos++;
    }

    double value = strtod(pos, NULL);
    return value;
}

// Extract string from JSON response (handles quoted strings)
void *__pluto_rpc_extract_string(void *response_ptr) {
    long *header = (long *)response_ptr;
    long len = header[0];
    const char *data = (const char *)&header[1];

    const char *result_key = "\"result\":\"";
    const char *pos = strstr(data, result_key);
    if (!pos) {
        fprintf(stderr, "RPC Error: could not find 'result' in JSON response\n");
        exit(1);
    }

    // Skip the "result":" part
    pos += strlen(result_key);

    // Find the closing quote
    const char *end = strchr(pos, '"');
    if (!end) {
        fprintf(stderr, "RPC Error: malformed string in JSON response\n");
        exit(1);
    }

    // Extract the string value
    long str_len = end - pos;
    char *str_buf = (char *)malloc(str_len + 1);
    memcpy(str_buf, pos, str_len);
    str_buf[str_len] = '\0';

    void *result = __pluto_make_string(str_buf);
    free(str_buf);
    return result;
}

// Extract bool from JSON response
long __pluto_rpc_extract_bool(void *response_ptr) {
    long *header = (long *)response_ptr;
    long len = header[0];
    const char *data = (const char *)&header[1];

    const char *result_key = "\"result\":";
    const char *pos = strstr(data, result_key);
    if (!pos) {
        fprintf(stderr, "RPC Error: could not find 'result' in JSON response\n");
        exit(1);
    }

    pos += strlen(result_key);

    // Check for "true" or "false"
    if (strncmp(pos, "true", 4) == 0) {
        return 1;
    } else if (strncmp(pos, "false", 5) == 0) {
        return 0;
    } else {
        fprintf(stderr, "RPC Error: expected boolean in JSON response\n");
        exit(1);
    }
}
