long long primes(long long n) {
    if (n <= 2) {
        return 0;
    }
    static char sieve[2001];
    long long limit = n;
    for (long long i = 0; i < limit; i++) {
        sieve[i] = 1;
    }
    sieve[0] = 0;
    sieve[1] = 0;
    for (long long i = 2; i * i < limit; i++) {
        if (sieve[i]) {
            for (long long j = i * i; j < limit; j += i) {
                sieve[j] = 0;
            }
        }
    }
    long long count = 0;
    for (long long i = 2; i < limit; i++) {
        if (sieve[i]) {
            count++;
        }
    }
    return count;
}
