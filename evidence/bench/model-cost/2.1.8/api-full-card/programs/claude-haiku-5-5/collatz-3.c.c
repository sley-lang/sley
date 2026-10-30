long long collatz(long long limit)
{
    if (limit <= 1) {
        return 0;
    }

    long long best = 1;
    long long best_steps = 0;

    for (long long n = 1; n < limit; n++) {
        unsigned long long v = (unsigned long long)n;
        long long steps = 0;
        while (v != 1) {
            if (v % 2 == 0) {
                v /= 2;
            } else {
                v = 3 * v + 1;
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
