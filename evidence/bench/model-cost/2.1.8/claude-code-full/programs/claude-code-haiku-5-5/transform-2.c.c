#include <stddef.h>
#include <limits.h>

long long transform(const long long *xs, size_t xs_len, long long lo, long long hi) {
    long long sum = 0;
    for (size_t i = 0; i < xs_len; i++) {
        long long x = xs[i];
        long long v;
        if (x % 2 != 0) {
            if (x > LLONG_MAX / 2) {
                v = hi;
            } else if (x < LLONG_MIN / 2) {
                v = lo;
            } else {
                v = 2 * x;
            }
        } else {
            v = x;
        }
        if (v < lo) v = lo;
        if (v > hi) v = hi;
        sum += v;
    }
    return sum;
}
