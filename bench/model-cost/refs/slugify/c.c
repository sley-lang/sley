#include <stddef.h>

void slugify(const char *s, char *out)
{
    size_t pos = 0;
    int pending_dash = 0;

    for (size_t i = 0; s[i] != '\0'; i++) {
        char c = s[i];
        int alnum = (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9');
        if (alnum) {
            if (pending_dash && pos > 0)
                out[pos++] = '-';
            pending_dash = 0;
            if (c >= 'A' && c <= 'Z')
                c = (char)(c - 'A' + 'a');
            out[pos++] = c;
        } else {
            pending_dash = 1;
        }
    }
    out[pos] = '\0';
}
