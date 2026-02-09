#include <stdio.h>
#include <math.h>
#include <time.h>

static double eval_a(int i, int j) {
    int ij = i + j;
    return 1.0 / (ij * (ij + 1) / 2 + i + 1);
}

static void eval_a_times_u(const double *u, double *au, int n) {
    for (int i = 0; i < n; i++) {
        double sum = 0;
        for (int j = 0; j < n; j++) sum += eval_a(i, j) * u[j];
        au[i] = sum;
    }
}

static void eval_at_times_u(const double *u, double *atu, int n) {
    for (int i = 0; i < n; i++) {
        double sum = 0;
        for (int j = 0; j < n; j++) sum += eval_a(j, i) * u[j];
        atu[i] = sum;
    }
}

static void eval_ata_times_u(const double *u, double *atau, double *tmp, int n) {
    eval_a_times_u(u, tmp, n);
    eval_at_times_u(tmp, atau, n);
}

int main(void) {
    int n = 500;
    double u[500], v[500], tmp[500];

    for (int i = 0; i < n; i++) u[i] = 1.0;

    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);

    for (int i = 0; i < 10; i++) {
        eval_ata_times_u(u, v, tmp, n);
        eval_ata_times_u(v, u, tmp, n);
    }

    double vbv = 0, vv = 0;
    for (int i = 0; i < n; i++) {
        vbv += u[i] * v[i];
        vv += v[i] * v[i];
    }

    clock_gettime(CLOCK_MONOTONIC, &t1);
    long ms = (t1.tv_sec - t0.tv_sec) * 1000 + (t1.tv_nsec - t0.tv_nsec) / 1000000;
    printf("spectral norm: %f\n", sqrt(vbv / vv));
    printf("elapsed: %ld ms\n", ms);
    return 0;
}
