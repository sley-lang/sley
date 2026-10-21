#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static long long fib(long long n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }

static long long loopsum(long long n) {
    long long s = 0;
    for (long long i = 0; i < n; i++) s = (s + i * i) % 1000000007;
    return s;
}

static long long collatz(long long limit) {
    long long best = 0, best_n = 0;
    for (long long n = 1; n < limit; n++) {
        long long x = n, steps = 0;
        while (x != 1) {
            x = x % 2 == 0 ? x / 2 : 3 * x + 1;
            steps++;
        }
        if (steps > best) { best = steps; best_n = n; }
    }
    return best_n;
}

static long long primes(long long n) {
    long long count = 0;
    for (long long k = 2; k < n; k++) {
        int is_p = 1;
        for (long long d = 2; d * d <= k; d++)
            if (k % d == 0) { is_p = 0; break; }
        count += is_p;
    }
    return count;
}

static long long gcd(long long a, long long b) {
    while (b != 0) { long long t = a % b; a = b; b = t; }
    return a;
}

static long long gcdsum(long long n) {
    long long s = 0;
    for (long long i = 1; i < n; i++)
        for (long long j = 1; j < n; j++) s += gcd(i, j);
    return s;
}

int main(int argc, char **argv) {
    const char *name = argv[1];
    long long n = atoll(argv[2]), r;
    if (!strcmp(name, "fib")) r = fib(n);
    else if (!strcmp(name, "loopsum")) r = loopsum(n);
    else if (!strcmp(name, "collatz")) r = collatz(n);
    else if (!strcmp(name, "primes")) r = primes(n);
    else if (!strcmp(name, "gcdsum")) r = gcdsum(n);
    else return 2;
    printf("%lld\n", r);
    return 0;
}
