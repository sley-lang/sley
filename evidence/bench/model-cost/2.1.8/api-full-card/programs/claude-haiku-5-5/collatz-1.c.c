#include <stdlib.h>
#include <stdint.h>

long long collatz(long long limit) {
    if (limit <= 1) {
        return 0;
    }

    uint32_t *steps = calloc((size_t)limit, sizeof(uint32_t));
    if (steps == NULL) {
        return 0;
    }

    /* steps[1] = 0 already from calloc */
    long long best = 1;
    uint32_t bestSteps = 0;

    for (long long n = 2; n < limit; n++) {
        unsigned long long x = (unsigned long long)n;
        uint64_t total = 0;

        /* Walk until we drop below n, whose step count is already known. */
        do {
            if (x % 2 == 0) {
                x /= 2;
            } else {
                x = 3ULL * x + 1ULL;
            }
            total++;
        } while (x >= (unsigned long long)n);

        total += steps[x];
        steps[n] = (uint32_t)total;

        if ((uint32_t)total > bestSteps) {
            bestSteps = (uint32_t)total;
            best = n;
        }
    }

    free(steps);
    return best;
}
