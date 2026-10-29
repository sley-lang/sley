#include <stddef.h>
#include <limits.h>

long long transform(const long long *xs, size_t xs_len, long long lo, long long hi) {
    long long total = 0;
    for (size_t i = 0; i < xs_len; i++) {
        long long x = xs[i];
        long long y;
        if (x % 2 != 0) {
            /* Doubling could overflow; if so, the value is far outside
               [lo, hi] anyway, so clamp it to the nearest bound directly. */
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
        if (y < lo) {
            y = lo;
        } else if (y > hi) {
            y = hi;
        }
        total += y;
    }
    return total;
}
