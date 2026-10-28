long long isqrt(long long n) {
    long long total = 0;
    for (long long i = 0; i < n; i++) {
        long long lo = 0, hi = i + 1;
        while (hi - lo > 1) {
            long long mid = (lo + hi) / 2;
            if (mid * mid <= i)
                lo = mid;
            else
                hi = mid;
        }
        total += lo;
    }
    return total;
}
