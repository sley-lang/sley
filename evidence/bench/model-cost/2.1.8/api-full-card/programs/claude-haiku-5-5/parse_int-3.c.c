#include <stddef.h>
#include <limits.h>

int parse_int(const long long *chars, size_t chars_len, long long *out)
{
    if (chars == NULL || chars_len == 0 || out == NULL) {
        return 1;
    }

    size_t i = 0;
    int negative = 0;

    if (chars[0] == 45) {
        negative = 1;
        i = 1;
        if (chars_len == 1) {
            return 1;
        }
    }

    /* Accumulate as a non-positive value so LLONG_MIN is representable. */
    long long acc = 0;
    for (; i < chars_len; i++) {
        long long c = chars[i];
        if (c < 48 || c > 57) {
            return 1;
        }
        long long d = c - 48;

        if (acc < LLONG_MIN / 10) {
            return 1;
        }
        long long t = acc * 10;
        if (t < LLONG_MIN + d) {
            return 1;
        }
        acc = t - d;
    }

    if (negative) {
        *out = acc;
    } else {
        if (acc == LLONG_MIN) {
            return 1;
        }
        *out = -acc;
    }
    return 0;
}
