long long root_sum(long long n) {
    long long total = 0;
    for (long long k = 1; k <= n; k++) {
        long long x = k;
        while (x >= 10) {
            long long s = 0;
            while (x > 0) {
                s += x % 10;
                x /= 10;
            }
            x = s;
        }
        total += x;
    }
    return total;
}
