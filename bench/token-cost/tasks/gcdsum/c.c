long long gcd(long long a, long long b) {
    while (b != 0) {
        long long t = a % b;
        a = b;
        b = t;
    }
    return a;
}

long long gcdsum(long long n) {
    long long s = 0;
    for (long long i = 1; i < n; i++)
        for (long long j = 1; j < n; j++)
            s += gcd(i, j);
    return s;
}
