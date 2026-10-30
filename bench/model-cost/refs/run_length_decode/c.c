#include <stddef.h>

int run_length_decode(const char *s, char *out)
{
    size_t i = 0;
    size_t total = 0;
    char buf[1001];

    while (s[i] != '\0') {
        char c = s[i];
        long long count = 0;

        if (c < 0x20 || c > 0x7e || (c >= '0' && c <= '9'))
            return 1;
        i++;
        if (s[i] < '1' || s[i] > '9')
            return 1;
        while (s[i] >= '0' && s[i] <= '9') {
            if (count <= 1000)
                count = count * 10 + (s[i] - '0');
            i++;
        }
        if (count > 1000 || total + (size_t)count > 1000)
            return 1;
        for (long long j = 0; j < count; j++)
            buf[total++] = c;
    }
    for (size_t j = 0; j < total; j++)
        out[j] = buf[j];
    out[total] = '\0';
    return 0;
}
