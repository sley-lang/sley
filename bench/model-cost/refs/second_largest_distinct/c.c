#include <stddef.h>

int second_largest_distinct(const long long *xs, size_t xs_len, long long *out)
{
    long long max;
    long long second = 0;
    int have_second = 0;

    if (xs_len == 0)
        return 1;
    max = xs[0];
    for (size_t i = 1; i < xs_len; i++) {
        if (xs[i] > max)
            max = xs[i];
    }
    for (size_t i = 0; i < xs_len; i++) {
        if (xs[i] < max && (!have_second || xs[i] > second)) {
            second = xs[i];
            have_second = 1;
        }
    }
    if (!have_second)
        return 1;
    *out = second;
    return 0;
}
