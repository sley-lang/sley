long long collatz(long long limit)
{
    if (limit <= 1)
        return 0;

    long long best_n = 1;
    long long best_steps = 0;

    for (long long n = 1; n < limit; n++) {
        unsigned long long x = (unsigned long long)n;
        long long steps = 0;
        while (x != 1) {
            if (x % 2 == 0)
                x /= 2;
            else
                x = 3 * x + 1;
            steps++;
        }
        if (steps > best_steps) {
            best_steps = steps;
            best_n = n;
        }
    }

    return best_n;
}
