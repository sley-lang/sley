#include <stddef.h>

long long fib(long long n) {
    if (n <= 1) {
        return n;
    }
    long long a = 0;
    long long b = 1;
    for (long long i = 2; i <= n; i++) {
        long long next = a + b;
        a = b;
        b = next;
    }
    return b;
}
