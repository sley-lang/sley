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

    if (i >= chars_len) {
        return 1; /* only a minus sign */
    }

    /* Largest magnitude representable: LLONG_MAX, or LLONG_MAX + 1 for negatives. */
    unsigned long long limit = (unsigned long long)LLONG_MAX;
    if (neg) {
        limit += 1ULL;
    }

    unsigned long long mag = 0;
    for (; i < chars_len; i++) {
        long long c = chars[i];
        if (c < 48 || c > 57) {
            return 1;
        }
        unsigned long long d = (unsigned long long)(c - 48);
        if (mag > (limit - d) / 10ULL) {
            return 1; /* overflow */
        }
        mag = mag * 10ULL + d;
    }

    if (neg) {
        *out = (mag == limit) ? LLONG_MIN : -(long long)mag;
    } else {
        *out = (long long)mag;
    }
    return 0;
}
