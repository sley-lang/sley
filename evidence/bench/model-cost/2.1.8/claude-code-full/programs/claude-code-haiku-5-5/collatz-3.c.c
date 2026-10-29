#include <stddef.h>

long long collatz(long long limit) {
    if (limit <= 1) {
        return 0;
    }

    long long best = 1;
    long long best_steps = 0;

    for (long long n = 2; n < limit; n++) {
        unsigned long long x = (unsigned long long)n;
        long long steps = 0;

        while (x != 1) {
            if (x & 1ULL) {
                x = 3ULL * x + 1ULL;
            } else {
                x >>= 1;
            }
            steps++;
        }

        if (steps > best_steps) {
            best_steps = steps;
            best = n;
        }
    }

    return best;
}
