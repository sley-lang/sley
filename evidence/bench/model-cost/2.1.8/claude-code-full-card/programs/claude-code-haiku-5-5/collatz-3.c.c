#include <stddef.h>

long long collatz(long long limit)
{
    if (limit <= 1)
        return 0;

    long long best_n = 1;
    long long best_steps = 0;

    for (long long start = 1; start < limit; start++) {
        unsigned long long x = (unsigned long long)start;
        long long steps = 0;

        while (x != 1) {
            if ((x & 1ULL) == 0)
                x >>= 1;
            else
                x = 3ULL * x + 1ULL;
            steps++;
        }

        if (steps > best_steps) {
            best_steps = steps;
            best_n = start;
        }
    }

    return best_n;
}
