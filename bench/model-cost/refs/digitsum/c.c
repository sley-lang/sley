long long digitsum(long long n) {
    long long total = 0;
    for (long long i = 0; i < n; i++)
        for (long long x = i; x > 0; x /= 10)
            total += x % 10;
    return total;
}
