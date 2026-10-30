#include <string.h>

static int is_digit(char c)
{
    return c >= '0' && c <= '9';
}

int parse_clock_seconds(const char *s, long long *out)
{
    long long hh, mm, ss;

    if (strlen(s) != 8)
        return 1;
    if (!is_digit(s[0]) || !is_digit(s[1]) || s[2] != ':' ||
        !is_digit(s[3]) || !is_digit(s[4]) || s[5] != ':' ||
        !is_digit(s[6]) || !is_digit(s[7]))
        return 1;
    hh = (s[0] - '0') * 10 + (s[1] - '0');
    mm = (s[3] - '0') * 10 + (s[4] - '0');
    ss = (s[6] - '0') * 10 + (s[7] - '0');
    if (hh > 23 || mm > 59 || ss > 59)
        return 1;
    *out = hh * 3600 + mm * 60 + ss;
    return 0;
}
