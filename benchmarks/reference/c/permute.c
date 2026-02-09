#include <stdio.h>
#include <time.h>

static long time_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000000000L + ts.tv_nsec;
}

void swap(long *arr, long i, long j) {
    long tmp = arr[i];
    arr[i] = arr[j];
    arr[j] = tmp;
}

long permute(long *arr, long n, long count) {
    if (n == 1) return count + 1;
    for (long i = 0; i < n; i++) {
        count = permute(arr, n - 1, count);
        if (n % 2 == 0) {
            swap(arr, i, n - 1);
        } else {
            swap(arr, 0, n - 1);
        }
    }
    return count;
}

int main(void) {
    long size = 10;
    long start = time_ns();

    long arr[10];
    for (long i = 0; i < size; i++) arr[i] = i;
    long count = permute(arr, size, 0);

    long elapsed = time_ns() - start;
    long ms = elapsed / 1000000;
    printf("permutations of %ld: %ld\n", size, count);
    printf("elapsed: %ld ms\n", ms);
    return 0;
}
