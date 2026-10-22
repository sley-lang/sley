long long loopsum(long long n) {
    long long s = 0;
    for (long long i = 0; i < n; i++)
        s = (s + i * i) % 1000000007;
    return s;
}
