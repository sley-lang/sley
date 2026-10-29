long long isqrt(long long n) {
    long long total = 0;
    for (long long i = 0; i < n; i++) {
        long long r = 0;
        while ((r + 1) * (r + 1) <= i) {
            r++;
        }
        total += r;
    }
    return total;
}
