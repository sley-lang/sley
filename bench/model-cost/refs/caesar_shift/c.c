#include <stddef.h>

void caesar_shift(const char *s, long long k, char *out)
{
    long long shift = ((k % 26) + 26) % 26;
    size_t i;

    for (i = 0; s[i] != '\0'; i++) {
        char c = s[i];
        if (c >= 'a' && c <= 'z')
            c = (char)('a' + (c - 'a' + shift) % 26);
        else if (c >= 'A' && c <= 'Z')
            c = (char)('A' + (c - 'A' + shift) % 26);
        out[i] = c;
    }
    out[i] = '\0';
}
