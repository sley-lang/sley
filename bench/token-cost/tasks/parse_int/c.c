#include <stddef.h>

enum parse_error { PARSE_OK, PARSE_EMPTY, PARSE_LONE_MINUS, PARSE_NOT_DIGIT };

enum parse_error parse_int(const int *chars, size_t len, long long *out) {
    if (len == 0)
        return PARSE_EMPTY;
    long long sign = 1;
    size_t start = 0;
    if (chars[0] == '-') {
        sign = -1;
        start = 1;
        if (len == 1)
            return PARSE_LONE_MINUS;
    }
    long long value = 0;
    for (size_t i = start; i < len; i++) {
        int c = chars[i];
        if (c < '0' || c > '9')
            return PARSE_NOT_DIGIT;
        value = value * 10 + (c - '0');
    }
    *out = sign * value;
    return PARSE_OK;
}
