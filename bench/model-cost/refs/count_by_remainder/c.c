#include <stddef.h>

int count_by_remainder(const long long *xs, size_t xs_len, long long m, long long *out, size_t *out_len)
{
    if (m < 1 || m > 200)
        return 1;
    for (long long r = 0; r < m; r++)
        out[r] = 0;
    for (size_t i = 0; i < xs_len; i++) {
        long long r = ((xs[i] % m) + m) % m;
        out[r]++;
    }
    *out_len = (size_t)m;
    return 0;
}
