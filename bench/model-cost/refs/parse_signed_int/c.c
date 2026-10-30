#include <stddef.h>

int parse_signed_int(const char *s, long long *out)
{
    size_t i = 0;
    int neg = 0;
    long long v = 0;
    int big = 0;

    if (s[i] == '+' || s[i] == '-') {
        neg = (s[i] == '-');
        i++;
    }
    if (s[i] == '\0')
        return 1;
    for (; s[i] != '\0'; i++) {
        if (s[i] < '0' || s[i] > '9')
            return 1;
        if (!big) {
            v = v * 10 + (s[i] - '0');
            if (v > 1000000)
                big = 1;
        }
    }
    if (big)
        return 1;
    *out = neg ? -v : v;
    return 0;
}
