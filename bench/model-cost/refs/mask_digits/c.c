#include <stddef.h>

void mask_digits(const char *s, char *out)
{
    size_t total = 0;
    size_t seen = 0;
    size_t i;

    for (i = 0; s[i] != '\0'; i++) {
        if (s[i] >= '0' && s[i] <= '9')
            total++;
    }
    for (i = 0; s[i] != '\0'; i++) {
        char c = s[i];
        if (c >= '0' && c <= '9') {
            if (total > 4 && seen < total - 4)
                c = '*';
            seen++;
        }
        out[i] = c;
    }
    out[i] = '\0';
}
