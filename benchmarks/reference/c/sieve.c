#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <stdbool.h>

static long time_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000000000L + ts.tv_nsec;
}

long sieve(long n) {
    bool *flags = malloc(n * sizeof(bool));
    for (long i = 0; i < n; i++) flags[i] = true;

    long count = 0;
    for (long p = 2; p < n; p++) {
        if (flags[p]) {
            count++;
            for (long m = p + p; m < n; m += p) {
                flags[m] = false;
            }
        }
    }
    free(flags);
    return count;
}

int main(void) {
    long n = 500000;
    long start = time_ns();
    long result = sieve(n);
    long elapsed = time_ns() - start;
    long ms = elapsed / 1000000;
    printf("primes below %ld: %ld\n", n, result);
    printf("elapsed: %ld ms\n", ms);
    return 0;
}
