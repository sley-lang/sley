#include <stddef.h>

int max_subarray(const long long *xs, size_t xs_len, long long *out) {
    if (xs_len == 0) return 1;
    long long best = xs[0], cur = xs[0];
    for (size_t i = 1; i < xs_len; i++) {
        cur = cur + xs[i] > xs[i] ? cur + xs[i] : xs[i];
        if (cur > best) best = cur;
    }
    *out = best;
    return 0;
}
