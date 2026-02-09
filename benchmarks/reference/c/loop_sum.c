#include <stdio.h>
#include <time.h>

static long time_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000000000L + ts.tv_nsec;
}

int main(void) {
    long start = time_ns();
    long sum = 0;
    long i = 0;
    while (i < 100000000) {
        sum += i;
        i++;
    }
    long elapsed = time_ns() - start;
    long ms = elapsed / 1000000;
    printf("sum = %ld\n", sum);
    printf("elapsed: %ld ms\n", ms);
    return 0;
}
