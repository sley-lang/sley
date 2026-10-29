#include <stddef.h>
#include <limits.h>

int max_subarray(const long long *xs, size_t xs_len, long long *out) {
    if (xs == NULL || xs_len == 0 || out == NULL) {
        return 1;
    }

    long long best = xs[0];
    long long cur = xs[0];

    for (size_t i = 1; i < xs_len; i++) {
        long long x = xs[i];
        if (cur <= 0) {
            /* Restarting at x never overflows and is at least as good as extending. */
            cur = x;
        } else {
            /* cur > 0: extending adds a positive value, so check for overflow first. */
            if (x > LLONG_MAX - cur) {
                return 1;
            }
            cur += x;
        }
        if (cur > best) {
            best = cur;
        }
    }

    *out = best;
    return 0;
}
