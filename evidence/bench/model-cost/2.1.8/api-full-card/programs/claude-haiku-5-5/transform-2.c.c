#include <stddef.h>
#include <limits.h>

static long long clamp_ll(long long v, long long lo, long long hi) {
    if (v < lo) return lo;
    if (v > hi) return hi;
    return v;
}

long long transform(const long long *xs, size_t xs_len, long long lo, long long hi) {
    unsigned long long total = 0;
    for (size_t i = 0; i < xs_len; i++) {
        long long x = xs[i];
        long long y;
        if (x % 2 != 0) {
            if (x > LLONG_MAX / 2) {
                y = hi;
            } else if (x < LLONG_MIN / 2) {
                y = lo;
            } else {
                y = clamp_ll(2 * x, lo, hi);
            }
        } else {
            y = clamp_ll(x, lo, hi);
        }
        total += (unsigned long long)y;
    }
    return (long long)total;
}
