#include <stddef.h>

int second_largest(const long long *xs, size_t xs_len, long long *out) {
    if (xs == NULL || out == NULL || xs_len < 2) {
        return 1;
    }

    int have_max = 0;
    int have_second = 0;
    long long mx = 0;
    long long sec = 0;

    for (size_t i = 0; i < xs_len; i++) {
        long long v = xs[i];

        if (!have_max) {
            mx = v;
            have_max = 1;
        } else if (v > mx) {
            /* The old maximum is distinct from v and becomes the runner-up. */
            sec = mx;
            have_second = 1;
            mx = v;
        } else if (v < mx) {
            if (!have_second || v > sec) {
                sec = v;
                have_second = 1;
            }
        }
        /* v == mx: duplicate of the maximum, ignore. */
    }

    if (!have_second) {
        return 1;
    }

    *out = sec;
    return 0;
}
