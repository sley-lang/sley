#include <stddef.h>

long long longest_run(const long long *xs, size_t xs_len)
{
    if (xs == NULL || xs_len == 0) {
        return 0;
    }

    long long best = 1;
    long long current = 1;

    for (size_t i = 1; i < xs_len; i++) {
        if (xs[i] == xs[i - 1]) {
            current++;
        } else {
            current = 1;
        }
        if (current > best) {
            best = current;
        }
    }

    return best;
}
