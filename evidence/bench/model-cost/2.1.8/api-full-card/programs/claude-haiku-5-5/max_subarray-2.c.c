#include <stddef.h>

int max_subarray(const long long *xs, size_t xs_len, long long *out) {
    if (xs == NULL || out == NULL || xs_len == 0) {
        return 1;
    }

    long long best = xs[0];
    long long cur = xs[0];

    for (size_t i = 1; i < xs_len; i++) {
        long long sum;
        if (__builtin_add_overflow(cur, xs[i], &sum)) {
            return 1;
        }
        cur = (xs[i] > sum) ? xs[i] : sum;
        if (cur > best) {
            best = cur;
        }
    }

    *out = best;
    return 0;
}
