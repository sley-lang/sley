#include <stddef.h>

long long longest_increasing_run(const long long *xs, size_t xs_len)
{
    long long best = 0;
    long long cur = 0;

    for (size_t i = 0; i < xs_len; i++) {
        if (i > 0 && xs[i] > xs[i - 1])
            cur++;
        else
            cur = 1;
        if (cur > best)
            best = cur;
    }
    return best;
}
