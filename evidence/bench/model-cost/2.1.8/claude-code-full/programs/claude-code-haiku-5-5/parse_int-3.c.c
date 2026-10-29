#include <stddef.h>
#include <limits.h>

int parse_int(const long long *chars, size_t chars_len, long long *out) {
    if (chars == NULL || chars_len == 0 || out == NULL) {
        return 1;
    }

    size_t i = 0;
    int neg = 0;
    if (chars[0] == 45) {
        neg = 1;
        i = 1;
    }
    if (i >= chars_len) {
        return 1;
    }

    /* Accumulate as a non-positive value so LLONG_MIN is representable. */
    long long acc = 0;
    for (; i < chars_len; i++) {
        long long c = chars[i];
        if (c < 48 || c > 57) {
            return 1;
        }
        int d = (int)(c - 48);
        if (acc < (LLONG_MIN + d) / 10) {
            return 1; /* would overflow */
        }
        acc = acc * 10 - d;
    }

    if (neg) {
        *out = acc;
    } else {
        if (acc == LLONG_MIN) {
            return 1; /* magnitude exceeds LLONG_MAX */
        }
        *out = -acc;
    }
    return 0;
}
