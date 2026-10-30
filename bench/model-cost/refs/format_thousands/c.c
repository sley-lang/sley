#include <stddef.h>

void format_thousands(long long n, char *out)
{
    char digits[32];
    size_t nd = 0;
    size_t pos = 0;
    unsigned long long v;

    if (n < 0) {
        out[pos++] = '-';
        v = (unsigned long long)(-(n + 1)) + 1;
    } else {
        v = (unsigned long long)n;
    }
    do {
        digits[nd++] = (char)('0' + v % 10);
        v /= 10;
    } while (v > 0);

    for (size_t i = nd; i > 0; i--) {
        out[pos++] = digits[i - 1];
        if (i - 1 > 0 && (i - 1) % 3 == 0)
            out[pos++] = ',';
    }
    out[pos] = '\0';
}
