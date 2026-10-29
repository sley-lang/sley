#include <stddef.h>

static long long digital_root(long long x) {
    while (x >= 10) {
        long long s = 0;
        while (x > 0) {
            s += x % 10;
            x /= 10;
        }
        x = s;
    }
    return x;
}

long long root_sum(long long n) {
    long long total = 0;
    for (long long i = 1; i <= n; i++) {
        total += digital_root(i);
    }
    return total;
}
