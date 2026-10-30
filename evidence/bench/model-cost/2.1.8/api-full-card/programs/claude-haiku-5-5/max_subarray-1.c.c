#include <stddef.h>

int max_subarray(const long long *xs, size_t xs_len, long long *out) {
    if (xs == NULL || xs_len == 0 || out == NULL) {
        return 1;
    }

    long long best = xs[0];
    long long cur = xs[0];

    for (size_t i = 1; i < xs_len; i++) {
        long long next;
        /* Extend the current run, or start fresh at xs[i] if that is larger. */
        if (__builtin_add_overflow(cur, xs[i], &next)) {
            return 1;
        }
        cur = (next > xs[i]) ? next : xs[i];
        if (cur > best) {
            best = cur;
        }
    }

    *out = best;
    return 0;
}
