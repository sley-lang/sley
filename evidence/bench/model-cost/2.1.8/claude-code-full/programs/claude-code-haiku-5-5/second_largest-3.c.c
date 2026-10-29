#include <stddef.h>

int second_largest(const long long *xs, size_t xs_len, long long *out) {
    if (xs == NULL || out == NULL || xs_len == 0) {
        return 1;
    }

    int has_max = 0;
    int has_second = 0;
    long long max = 0;
    long long second = 0;

    for (size_t i = 0; i < xs_len; i++) {
        long long x = xs[i];

        if (!has_max) {
            max = x;
            has_max = 1;
        } else if (x > max) {
            if (has_second || max != x) {
                second = max;
                has_second = 1;
            }
            max = x;
        } else if (x < max) {
            if (!has_second || x > second) {
                second = x;
                has_second = 1;
            }
        }
        /* x == max: duplicate of the largest value, ignore */
    }

    if (!has_second) {
        return 1;
    }

    *out = second;
    return 0;
}
