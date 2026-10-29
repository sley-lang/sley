#include <stddef.h>
#include <limits.h>

long long transform(const long long *xs, size_t xs_len, long long lo, long long hi) {
    __int128 total = 0;
    for (size_t i = 0; i < xs_len; i++) {
        long long x = xs[i];
        __int128 v;
        if (x % 2 != 0) {
            /* 2x, computed in 128-bit so it cannot overflow */
            v = (__int128)x * 2;
        } else {
            v = (__int128)x;
        }
        if (v < (__int128)lo) {
            v = (__int128)lo;
        } else if (v > (__int128)hi) {
            v = (__int128)hi;
        }
        total += v;
    }
    if (total > (__int128)LLONG_MAX) {
        return LLONG_MAX;
    }
    if (total < (__int128)LLONG_MIN) {
        return LLONG_MIN;
    }
    return (long long)total;
}
