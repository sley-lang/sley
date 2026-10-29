#include <stdlib.h>

long long collatz(long long limit)
{
    if (limit <= 1)
        return 0;

    /* cache[v] = number of steps for v to reach 1, for 1 <= v < limit */
    int *cache = calloc((size_t)limit, sizeof(int));
    if (cache == NULL)
        return 0;

    cache[1] = 0;
    long long best = 1;
    int best_steps = 0;

    for (long long n = 2; n < limit; n++) {
        long long x = n;
        int s = 0;
        /* Walk until the value drops below n, which is already cached. */
        while (x >= n) {
            if (x % 2 == 0)
                x /= 2;
            else
                x = 3 * x + 1;
            s++;
        }
        cache[n] = s + cache[x];

        if (cache[n] > best_steps) {
            best_steps = cache[n];
            best = n;
        }
    }

    free(cache);
    return best;
}
