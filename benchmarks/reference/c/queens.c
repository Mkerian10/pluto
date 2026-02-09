#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>
#include <time.h>

static long time_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000000000L + ts.tv_nsec;
}

long place_queen(long row, long n, bool *cols, bool *diag1, bool *diag2) {
    if (row == n) return 1;
    long count = 0;
    for (long col = 0; col < n; col++) {
        long d1 = row + col;
        long d2 = row - col + n - 1;
        if (!cols[col] && !diag1[d1] && !diag2[d2]) {
            cols[col] = true;
            diag1[d1] = true;
            diag2[d2] = true;
            count += place_queen(row + 1, n, cols, diag1, diag2);
            cols[col] = false;
            diag1[d1] = false;
            diag2[d2] = false;
        }
    }
    return count;
}

long queens_solve(long n) {
    bool *cols = calloc(n, sizeof(bool));
    bool *diag1 = calloc(2 * n, sizeof(bool));
    bool *diag2 = calloc(2 * n, sizeof(bool));
    long result = place_queen(0, n, cols, diag1, diag2);
    free(cols);
    free(diag1);
    free(diag2);
    return result;
}

int main(void) {
    long n = 12;
    long start = time_ns();
    long solutions = queens_solve(n);
    long elapsed = time_ns() - start;
    long ms = elapsed / 1000000;
    printf("%ld-queens solutions: %ld\n", n, solutions);
    printf("elapsed: %ld ms\n", ms);
    return 0;
}
