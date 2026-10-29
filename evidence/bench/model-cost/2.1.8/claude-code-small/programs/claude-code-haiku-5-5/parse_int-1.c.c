#include <stddef.h>

int parse_int(const long long *chars, size_t chars_len, long long *out) {
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

    long long value = 0;
    for (; i < chars_len; i++) {
        long long c = chars[i];
        if (c < 48 || c > 57) {
            return 1;
        }
        value = value * 10 + (c - 48);
    }

    *out = negative ? -value : value;
    return 0;
}
