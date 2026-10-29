#include <stddef.h>

int clamp_sum(const long long *xs, size_t xs_len, long long lo, long long hi, long long *out) {
    if (lo > hi) {
        return 1;
    }
    long long total = 0;
    for (size_t i = 0; i < xs_len; i++) {
        long long x = xs[i];
        if (x < 0) continue;
        if (x < lo) x = lo;
        if (x > hi) x = hi;
        total += x;
    }
    *out = total;
    return 0;
}
