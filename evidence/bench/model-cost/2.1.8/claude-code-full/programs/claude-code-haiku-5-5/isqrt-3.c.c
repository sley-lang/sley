long long isqrt(long long n) {
    long long sum = 0;
    long long r = 0;
    for (long long i = 0; i < n; i++) {
        while ((r + 1) * (r + 1) <= i) {
            r++;
        }
        sum += r;
    }
    return sum;
}
