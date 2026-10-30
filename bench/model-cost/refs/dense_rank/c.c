#include <stddef.h>

void dense_rank(const long long *xs, size_t xs_len, long long *out, size_t *out_len)
{
    long long distinct[200];
    size_t nd = 0;

    for (size_t i = 0; i < xs_len; i++) {
        size_t j;
        for (j = 0; j < nd; j++) {
            if (distinct[j] == xs[i])
                break;
        }
        if (j == nd)
            distinct[nd++] = xs[i];
    }
    for (size_t i = 0; i < xs_len; i++) {
        long long rank = 1;
        for (size_t j = 0; j < nd; j++) {
            if (distinct[j] > xs[i])
                rank++;
        }
        out[i] = rank;
    }
    *out_len = xs_len;
}
