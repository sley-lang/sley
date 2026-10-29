#include <stddef.h>

int parse_int(const long long *chars, size_t chars_len, long long *out) {
    if (chars_len == 0) return 1;
    long long sign = 1;
    size_t start = 0;
    if (chars[0] == 45) {
        if (chars_len == 1) return 1;
        sign = -1;
        start = 1;
    }
    long long value = 0;
    for (size_t i = start; i < chars_len; i++) {
        if (chars[i] < 48 || chars[i] > 57) return 1;
        value = value * 10 + (chars[i] - 48);
    }
    *out = sign * value;
    return 0;
}
