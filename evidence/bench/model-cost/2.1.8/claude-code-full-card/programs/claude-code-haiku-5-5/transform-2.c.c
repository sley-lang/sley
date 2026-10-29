#include <stddef.h>
#include <limits.h>

static long long clamp_ll(long long v, long long lo, long long hi) {
    if (v < lo) return lo;
    if (v > hi) return hi;
    return v;
}

long long transform(const long long *xs, size_t xs_len, long long lo, long long hi) {
    long long sum = 0;
    for (size_t i = 0; i < xs_len; i++) {
        long long x = xs[i];
        long long y;
        if (x % 2 != 0) {
            /* Odd: double, avoiding signed overflow by clamping the result directly. */
            if (x > LLONG_MAX / 2) {
                y = hi;
            } else if (x < LLONG_MIN / 2) {
                y = lo;
            } else {
                y = 2 * x;
            }
        } else {
            y = x;
        }
        sum += clamp_ll(y, lo, hi);
    }
    return sum;
}
