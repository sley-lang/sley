#include <stddef.h>

static long long floor_div(long long a, long long b)
{
    long long q = a / b;
    if ((a % b != 0) && ((a < 0) != (b < 0)))
        q--;
    return q;
}

int moving_average_floor(const long long *xs, size_t xs_len, long long w, long long *out, size_t *out_len)
{
    size_t n = 0;

    if (w < 1)
        return 1;
    if ((unsigned long long)w <= (unsigned long long)xs_len) {
        size_t ws = (size_t)w;
        for (size_t i = 0; i + ws <= xs_len; i++) {
            long long sum = 0;
            for (size_t j = i; j < i + ws; j++)
                sum += xs[j];
            out[n++] = floor_div(sum, w);
        }
    }
    *out_len = n;
    return 0;
}
