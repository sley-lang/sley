long long primes(long long n) {
    if (n <= 2) {
        return 0;
    }
    /* Sieve of Eratosthenes over [0, n). Values are small (n <= 2000). */
    char composite[2001] = {0};
    long long count = 0;
    for (long long i = 2; i < n; i++) {
        if (!composite[i]) {
            count++;
            for (long long j = i * i; j < n; j += i) {
                composite[j] = 1;
            }
        }
    }
    return count;
}
