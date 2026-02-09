#include <stdio.h>
#include <time.h>

int main(void) {
    int n = 2000;
    int max_iter = 50;

    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);

    long total_iters = 0;
    for (int y = 0; y < n; y++) {
        double ci = 2.0 * y / n - 1.0;
        for (int x = 0; x < n; x++) {
            double cr = 2.0 * x / n - 1.5;
            double zr = 0, zi = 0;
            int iter = 0;
            while (iter < max_iter) {
                double zr2 = zr * zr, zi2 = zi * zi;
                if (zr2 + zi2 > 4.0) break;
                zi = 2.0 * zr * zi + ci;
                zr = zr2 - zi2 + cr;
                iter++;
            }
            total_iters += iter;
        }
    }

    clock_gettime(CLOCK_MONOTONIC, &t1);
    long ms = (t1.tv_sec - t0.tv_sec) * 1000 + (t1.tv_nsec - t0.tv_nsec) / 1000000;
    printf("total iterations: %ld\n", total_iters);
    printf("elapsed: %ld ms\n", ms);
    return 0;
}
