#include <stddef.h>

int most_common_letter(const char *s, char *out)
{
    long long counts[26] = {0};
    int best = -1;

    for (size_t i = 0; s[i] != '\0'; i++) {
        char c = s[i];
        if (c >= 'A' && c <= 'Z')
            counts[c - 'A']++;
        else if (c >= 'a' && c <= 'z')
            counts[c - 'a']++;
    }
    for (int i = 0; i < 26; i++) {
        if (counts[i] > 0 && (best < 0 || counts[i] > counts[best]))
            best = i;
    }
    if (best < 0)
        return 1;
    out[0] = (char)('a' + best);
    out[1] = '\0';
    return 0;
}
