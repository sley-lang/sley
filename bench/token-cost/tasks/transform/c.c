#include <stddef.h>

long long transform(const long long *xs, size_t len, long long lo, long long hi) {
    long long s = 0;
    for (size_t i = 0; i < len; i++) {
        long long x = xs[i] % 2 ? xs[i] * 2 : xs[i];
        if (x < lo)
            x = lo;
        if (x > hi)
            x = hi;
        s += x;
    }
    return s;
}
