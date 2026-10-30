#include <string.h>

int to_roman(long long n, char *out)
{
    static const long long values[] = {1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1};
    static const char *const symbols[] = {"M", "CM", "D", "CD", "C", "XC", "L", "XL", "X", "IX", "V", "IV", "I"};
    size_t pos = 0;

    if (n < 1 || n > 3999)
        return 1;
    for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
        while (n >= values[i]) {
            size_t len = strlen(symbols[i]);
            memcpy(out + pos, symbols[i], len);
            pos += len;
            n -= values[i];
        }
    }
    out[pos] = '\0';
    return 0;
}
