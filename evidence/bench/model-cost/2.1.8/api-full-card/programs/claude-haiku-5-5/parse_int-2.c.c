#include <stddef.h>
#include <limits.h>

int parse_int(const long long *chars, size_t chars_len, long long *out) {
    if (chars == NULL || chars_len == 0 || out == NULL) {
        return 1;
    }

    size_t idx = 0;
    int neg = 0;
    if (chars[0] == 45) {
        neg = 1;
        idx = 1;
    }
    if (idx >= chars_len) {
        return 1;
    }

    /* Accumulate as a negative number to safely handle LLONG_MIN. */
    long long acc = 0;
    for (; idx < chars_len; idx++) {
        long long c = chars[idx];
        if (c < 48 || c > 57) {
            return 1;
        }
        long long d = c - 48;
        if (acc < LLONG_MIN / 10) {
            return 1;
        }
        acc *= 10;
        if (acc < LLONG_MIN + d) {
            return 1;
        }
        acc -= d;
    }

    if (neg) {
        *out = acc;
    } else {
        if (acc == LLONG_MIN) {
            return 1;
        }
        *out = -acc;
    }
    return 0;
}
