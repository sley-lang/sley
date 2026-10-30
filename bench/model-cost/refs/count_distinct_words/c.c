#include <string.h>

static int is_letter(char c)
{
    return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z');
}

static char to_lower(char c)
{
    return (c >= 'A' && c <= 'Z') ? (char)(c - 'A' + 'a') : c;
}

long long count_distinct_words(const char *s)
{
    char words[101][201];
    size_t nw = 0;
    size_t i = 0;

    while (s[i] != '\0') {
        if (!is_letter(s[i])) {
            i++;
            continue;
        }
        char w[201];
        size_t len = 0;
        while (is_letter(s[i]) && len < 200)
            w[len++] = to_lower(s[i++]);
        w[len] = '\0';

        size_t j;
        for (j = 0; j < nw; j++) {
            if (strcmp(words[j], w) == 0)
                break;
        }
        if (j == nw && nw < 101)
            strcpy(words[nw++], w);
    }
    return (long long)nw;
}
