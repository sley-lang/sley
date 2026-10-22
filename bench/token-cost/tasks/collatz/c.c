long long collatz(long long limit) {
    long long best = 0, best_n = 0;
    for (long long n = 1; n < limit; n++) {
        long long x = n, steps = 0;
        while (x != 1) {
            if (x % 2 == 0)
                x = x / 2;
            else
                x = 3 * x + 1;
            steps++;
        }
        if (steps > best) {
            best = steps;
            best_n = n;
        }
    }
    return best_n;
}
