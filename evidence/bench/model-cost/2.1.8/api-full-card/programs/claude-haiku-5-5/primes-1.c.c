long long primes(long long n) {
    if (n <= 2) {
        return 0;
    }
    static char sieve[2001];
    long long count = 0;
    for (long long i = 0; i < n && i <= 2000; i++) {
        sieve[i] = 1;
    }
    sieve[0] = 0;
    if (n > 1) sieve[1] = 0;
    for (long long i = 2; i * i < n; i++) {
        if (sieve[i]) {
            for (long long j = i * i; j < n; j += i) {
                sieve[j] = 0;
            }
        }
    }
    for (long long i = 2; i < n; i++) {
        if (sieve[i]) {
            count++;
        }
    }
    return count;
}
