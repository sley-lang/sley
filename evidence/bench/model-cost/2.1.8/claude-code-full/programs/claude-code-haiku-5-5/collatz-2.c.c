#include <stdlib.h>

long long collatz(long long limit)
{
    if (limit <= 1)
        return 0;

    /* cache[n] holds (steps to reach 1) + 1 for n < limit; 0 means not yet known. */
    unsigned int *cache = calloc((size_t)limit, sizeof *cache);
    if (cache == NULL)
        return 0;

    long long best_n = 1;
    long long best_steps = 0;

    for (long long n = 1; n < limit; n++) {
        unsigned long long x = (unsigned long long)n;
        long long s = 0;

        /* Walk until reaching 1 or a value with a known cached result. */
        while (x != 1 && (x >= (unsigned long long)limit || cache[x] == 0)) {
            x = (x % 2 == 0) ? x / 2 : 3 * x + 1;
            s++;
        }
        if (x != 1)
            s += (long long)cache[x] - 1;

        /* Record step counts for the nodes on this path that are below limit. */
        unsigned long long y = (unsigned long long)n;
        long long rem = s;
        while (y != 1 && y < (unsigned long long)limit && cache[y] == 0) {
            cache[y] = (unsigned int)(rem + 1);
            y = (y % 2 == 0) ? y / 2 : 3 * y + 1;
            rem--;
        }

        /* Strictly greater keeps the smallest n on ties, since n increases. */
        if (s > best_steps) {
            best_steps = s;
            best_n = n;
        }
    }

    free(cache);
    return best_n;
}
