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
        long long v;

        if (x % 2 != 0) {
            /* Odd: map to 2x, avoiding signed overflow.
               If doubling would overflow, the result is far outside
               [lo, hi] on the corresponding side, so clamp to that bound. */
            if (x > LLONG_MAX / 2) {
                v = hi;
            } else if (x < LLONG_MIN / 2) {
                v = lo;
            } else {
                v = 2 * x;
            }
        } else {
            v = x;
        }

        sum += clamp_ll(v, lo, hi);
    }

    return sum;
}
