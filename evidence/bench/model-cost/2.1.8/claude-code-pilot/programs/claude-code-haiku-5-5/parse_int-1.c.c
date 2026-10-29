#include <limits.h>
#include <stddef.h>

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
    if (i == chars_len) {
        return 1;
    }

    /* Accumulate in the negative domain for negative numbers so that
       LLONG_MIN can be represented without overflow. */
    long long acc = 0;
    for (; i < chars_len; i++) {
        long long c = chars[i];
        if (c < 48 || c > 57) {
            return 1;
        }
        int d = (int)(c - 48);
        if (neg) {
            if (acc < (LLONG_MIN + d) / 10) {
                return 1;
            }
            acc = acc * 10 - d;
        } else {
            if (acc > (LLONG_MAX - d) / 10) {
                return 1;
            }
            acc = acc * 10 + d;
        }
    }

    *out = acc;
    return 0;
}
