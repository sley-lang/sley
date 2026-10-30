#include <stddef.h>

static long long gcd(long long a, long long b)
{
    while (b != 0) {
        long long t = a % b;
        a = b;
        b = t;
    }
    return a;
}

long long gcd_of_list(const long long *xs, size_t xs_len)
{
    long long g = 0;

    for (size_t i = 0; i < xs_len; i++) {
        long long v = xs[i] < 0 ? -xs[i] : xs[i];
        g = gcd(g, v);
    }
    return g;
}
