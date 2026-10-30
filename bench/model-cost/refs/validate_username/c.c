#include <string.h>

long long validate_username(const char *s)
{
    size_t len = strlen(s);

    if (len < 3 || len > 16)
        return 0;
    if (s[0] < 'a' || s[0] > 'z')
        return 0;
    for (size_t i = 0; i < len; i++) {
        char c = s[i];
        int ok = (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '_';
        if (!ok)
            return 0;
        if (c == '_' && i > 0 && s[i - 1] == '_')
            return 0;
    }
    if (s[len - 1] == '_')
        return 0;
    return 1;
}
