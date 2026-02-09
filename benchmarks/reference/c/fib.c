#include <stdio.h>
#include <time.h>

static long time_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000000000L + ts.tv_nsec;
}

long fib(long n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}

int main(void) {
    long start = time_ns();
    long result = fib(35);
    long elapsed = time_ns() - start;
    long ms = elapsed / 1000000;
    printf("fib(35) = %ld\n", result);
    printf("elapsed: %ld ms\n", ms);
    return 0;
}
