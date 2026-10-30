#include <stddef.h>

long long count_pairs_with_sum(const long long *xs, size_t xs_len, long long target)
{
    long long count = 0;

    for (size_t i = 0; i < xs_len; i++) {
        for (size_t j = i + 1; j < xs_len; j++) {
            if (xs[i] + xs[j] == target)
                count++;
        }
    }
    return count;
}
