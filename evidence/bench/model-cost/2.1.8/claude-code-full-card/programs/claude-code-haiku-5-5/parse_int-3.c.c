#include <stddef.h>
#include <limits.h>

int parse_int(const long long *chars, size_t chars_len, long long *out) {
    if (chars == NULL || chars_len == 0 || out == NULL) {
        return 1;
    }

    size_t i = 0;
    int negative = 0;
    if (chars[0] == 45) {
        negative = 1;
        i = 1;
    }

    if (i == chars_len) {
        return 1;
    }

    long long acc = 0;
    for (; i < chars_len; i++) {
        long long c = chars[i];
        if (c < 48 || c > 57) {
            return 1;
        }
        int d = (int)(c - 48);

        if (negative) {
            /* Accumulate as a negative value so LLONG_MIN is representable. */
            if (acc < LLONG_MIN / 10) {
                return 1;
            }
            acc *= 10;
            if (acc < LLONG_MIN + d) {
                return 1;
            }
            acc -= d;
        } else {
            if (acc > LLONG_MAX / 10) {
                return 1;
            }
            acc *= 10;
            if (acc > LLONG_MAX - d) {
                return 1;
            }
            acc += d;
        }
    }

    *out = acc;
    return 0;
}
