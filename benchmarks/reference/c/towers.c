#include <stdio.h>
#include <time.h>

static long time_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000000000L + ts.tv_nsec;
}

long towers(long discs, long from, long to, long spare, long *pegs, long moves) {
    if (discs == 0) return moves;
    moves = towers(discs - 1, from, spare, to, pegs, moves);
    pegs[to]++;
    pegs[from]--;
    moves++;
    moves = towers(discs - 1, spare, to, from, pegs, moves);
    return moves;
}

int main(void) {
    long n = 20;
    long iters = 100;
    long start = time_ns();

    long total_moves = 0;
    for (long i = 0; i < iters; i++) {
        long pegs[3] = {n, 0, 0};
        long m = towers(n, 0, 1, 2, pegs, 0);
        total_moves += m;
    }

    long elapsed = time_ns() - start;
    long ms = elapsed / 1000000;
    printf("total moves: %ld\n", total_moves);
    printf("elapsed: %ld ms\n", ms);
    return 0;
}
