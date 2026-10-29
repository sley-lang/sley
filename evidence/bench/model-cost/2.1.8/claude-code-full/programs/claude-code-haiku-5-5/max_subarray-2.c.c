#include <stddef.h>

int max_subarray(const long long *xs, size_t xs_len, long long *out) {
    if (out == NULL || xs == NULL || xs_len == 0) {
        return 1;
    }

    long long best = xs[0];
    long long cur = xs[0];

    for (size_t i = 1; i < xs_len; i++) {
        long long extended;
        if (__builtin_add_overflow(cur, xs[i], &extended)) {
            return 1;
        }
        cur = (xs[i] > extended) ? xs[i] : extended;
        if (cur > best) {
            best = cur;
        }
    }

    *out = best;
    return 0;
}
