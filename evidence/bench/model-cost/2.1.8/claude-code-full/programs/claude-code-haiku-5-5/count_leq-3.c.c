#include <stddef.h>

long long count_leq(const long long *xs, size_t xs_len, long long x) {
    size_t lo = 0;
    size_t hi = xs_len;

    while (lo < hi) {
        size_t mid = lo + (hi - lo) / 2;
        if (xs[mid] <= x) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    return (long long)lo;
}
