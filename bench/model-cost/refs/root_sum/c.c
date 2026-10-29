long long root_sum(long long n) {
    long long total = 0;
    for (long long i = 1; i <= n; i++) total += 1 + (i - 1) % 9;
    return total;
}
