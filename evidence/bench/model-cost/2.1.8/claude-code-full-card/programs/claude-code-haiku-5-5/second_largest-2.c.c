#include <stddef.h>

int second_largest(const long long *xs, size_t xs_len, long long *out) {
    if (out == NULL || xs == NULL || xs_len == 0) {
        return 1;
    }

    long long max = xs[0];
    long long second = 0;
    int has_second = 0;

    for (size_t i = 1; i < xs_len; i++) {
        long long x = xs[i];
        if (x > max) {
            second = max;
            has_second = 1;
            max = x;
        } else if (x < max && (!has_second || x > second)) {
            second = x;
            has_second = 1;
        }
    }

    if (!has_second) {
        return 1;
    }

    *out = second;
    return 0;
}
