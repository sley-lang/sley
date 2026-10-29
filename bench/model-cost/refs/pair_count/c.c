#include <stddef.h>

long long pair_count(const long long *xs, size_t xs_len, long long k) {
    long long count = 0;
    for (size_t i = 0; i < xs_len; i++)
        for (size_t j = i + 1; j < xs_len; j++)
            if (xs[i] + xs[j] == k) count++;
    return count;
}
