#include <stdio.h>
#include <time.h>

int fannkuch(int n) {
    int perm[16], perm1[16], count[16], tmp;
    int max_flips = 0, checksum = 0, perm_count = 0;

    for (int i = 0; i < n; i++) perm1[i] = i;
    for (int i = 0; i < n; i++) count[i] = i + 1;

    while (1) {
        // Count flips for current permutation
        if (perm1[0] != 0) {
            for (int i = 0; i < n; i++) perm[i] = perm1[i];
            int flips = 0;
            while (perm[0] != 0) {
                int k = perm[0];
                int lo = 0, hi = k;
                while (lo < hi) {
                    tmp = perm[lo]; perm[lo] = perm[hi]; perm[hi] = tmp;
                    lo++; hi--;
                }
                flips++;
            }
            if (flips > max_flips) max_flips = flips;
            checksum += (perm_count % 2 == 0) ? flips : -flips;
        }
        perm_count++;

        // Generate next permutation
        int r;
        for (r = 1; r < n; r++) {
            int perm0 = perm1[0];
            for (int i = 0; i < r; i++) perm1[i] = perm1[i + 1];
            perm1[r] = perm0;
            if (--count[r] > 0) break;
            count[r] = r + 1;
        }
        if (r >= n) break;
    }

    printf("checksum: %d\n", checksum);
    return max_flips;
}

int main(void) {
    int n = 10;
    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);

    int result = fannkuch(n);

    clock_gettime(CLOCK_MONOTONIC, &t1);
    long ms = (t1.tv_sec - t0.tv_sec) * 1000 + (t1.tv_nsec - t0.tv_nsec) / 1000000;
    printf("max flips: %d\n", result);
    printf("elapsed: %ld ms\n", ms);
    return 0;
}
