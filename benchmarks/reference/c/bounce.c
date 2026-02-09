#include <stdio.h>
#include <time.h>

static long time_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000000000L + ts.tv_nsec;
}

long bounce_sim(long steps) {
    double x = 0.0, y = 0.0, vx = 1.5, vy = 2.3;
    double box_size = 100.0;
    long bounces = 0;

    for (long i = 0; i < steps; i++) {
        x += vx;
        y += vy;

        if (x < 0.0) {
            x = -x;
            vx = -vx;
            bounces++;
        }
        if (x > box_size) {
            x = box_size - (x - box_size);
            vx = -vx;
            bounces++;
        }
        if (y < 0.0) {
            y = -y;
            vy = -vy;
            bounces++;
        }
        if (y > box_size) {
            y = box_size - (y - box_size);
            vy = -vy;
            bounces++;
        }
    }
    return bounces;
}

int main(void) {
    long steps = 10000000;
    long start = time_ns();
    long bounces = bounce_sim(steps);
    long elapsed = time_ns() - start;
    long ms = elapsed / 1000000;
    printf("bounces: %ld\n", bounces);
    printf("elapsed: %ld ms\n", ms);
    return 0;
}
