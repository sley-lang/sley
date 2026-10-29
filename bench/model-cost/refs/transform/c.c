#include <stddef.h>

long long transform(const long long *xs, size_t xs_len, long long lo, long long hi) {
    long long total = 0;
    for (size_t i = 0; i < xs_len; i++) {
        long long x = xs[i] % 2 != 0 ? xs[i] * 2 : xs[i];
        if (x < lo) x = lo;
        if (x > hi) x = hi;
        total += x;
    }
    return total;
}
