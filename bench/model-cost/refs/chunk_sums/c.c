#include <stddef.h>

int chunk_sums(const long long *xs, size_t xs_len, long long size, long long *out, size_t *out_len)
{
    size_t n = 0;

    if (size < 1)
        return 1;
    for (size_t i = 0; i < xs_len; i += (size_t)size) {
        long long sum = 0;
        for (size_t j = i; j < xs_len && j - i < (size_t)size; j++)
            sum += xs[j];
        out[n++] = sum;
        if ((size_t)size > xs_len - i)
            break;
    }
    *out_len = n;
    return 0;
}
