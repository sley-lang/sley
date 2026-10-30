#include <stdlib.h>

long long collatz(long long limit) {
    if (limit <= 1) {
        return 0;
    }

    unsigned long long N = (unsigned long long)limit;
    /* cache[v] = steps(v) + 1 for processed starting values v < N; 0 means unknown. */
    unsigned int *cache = (unsigned int *)calloc((size_t)N, sizeof(unsigned int));

    unsigned long long best = 1;
    unsigned long long bestSteps = 0;

    if (cache != NULL) {
        cache[1] = 1; /* steps(1) = 0 */
        for (unsigned long long n = 2; n < N; n++) {
            unsigned long long x = n;
            unsigned long long steps = 0;
            while (x >= N || cache[x] == 0) {
                x = (x % 2 == 0) ? x / 2 : 3 * x + 1;
                steps++;
            }
            unsigned long long total = steps + (cache[x] - 1);
            cache[n] = (unsigned int)(total + 1);
            if (total > bestSteps) {
                bestSteps = total;
                best = n;
            }
        }
        free(cache);
    } else {
        /* Fallback without memoization. */
        for (unsigned long long n = 1; n < N; n++) {
            unsigned long long x = n;
            unsigned long long steps = 0;
            while (x != 1) {
                x = (x % 2 == 0) ? x / 2 : 3 * x + 1;
                steps++;
            }
            if (steps > bestSteps) {
                bestSteps = steps;
                best = n;
            }
        }
    }

    return (long long)best;
}
