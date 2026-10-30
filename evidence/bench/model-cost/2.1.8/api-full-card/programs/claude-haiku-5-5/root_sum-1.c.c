long long root_sum(long long n) {
    long long total = 0;
    for (long long i = 1; i <= n; i++) {
        long long d = i % 9;
        total += (d == 0) ? 9 : d;
    }
    return total;
}
