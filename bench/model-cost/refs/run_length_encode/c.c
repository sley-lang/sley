#include <stdio.h>
#include <stddef.h>

void run_length_encode(const char *s, char *out)
{
    size_t i = 0;
    size_t pos = 0;

    while (s[i] != '\0') {
        char c = s[i];
        size_t run = 0;
        while (s[i] == c) {
            run++;
            i++;
        }
        pos += (size_t)sprintf(out + pos, "%c%zu", c, run);
    }
    out[pos] = '\0';
}
