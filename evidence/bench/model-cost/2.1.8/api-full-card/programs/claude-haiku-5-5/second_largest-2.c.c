#include <stddef.h>

int second_largest(const long long *xs, size_t xs_len, long long *out) {
    if (xs == NULL || out == NULL || xs_len < 2) {
        return 1;
    }

    int have_max = 0;
    long long max = 0;
    long long second = 0;
    int have_second = 0;

    for (size_t i = 0; i < xs_len; i++) {
        long long v = xs[i];
        if (!have_max) {
            max = v;
            have_max = 1;
        } else if (v > max) {
            if (!have_second || max > second) {
                second = max;
                have_second = 1;
            }
            max = v;
        } else if (v < max) {
            if (!have_second || v > second) {
                second = v;
                have_second = 1;
            }
        }
    }

    if (!have_second) {
        return 1;
    }

    *out = second;
    return 0;
}
