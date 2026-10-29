long long primes(long long n) {
    long long count = 0;
    for (long long k = 2; k < n; k++) {
        int is_p = 1;
        for (long long d = 2; d * d <= k; d++) {
            if (k % d == 0) {
                is_p = 0;
                break;
            }
        }
        count += is_p;
    }
    return count;
}
