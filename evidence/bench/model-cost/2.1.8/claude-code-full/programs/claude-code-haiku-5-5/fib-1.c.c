long long fib(long long n) {
    long long a = 0, b = 1;
    for (long long i = 0; i < n; i++) {
        long long t = a + b;
        a = b;
        b = t;
    }
    return a;
}
