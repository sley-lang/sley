#include <stddef.h>

int mode_value(const long long *xs, size_t xs_len, long long *out) {
    if (xs_len == 0) return 1;
    long long best = xs[0], best_count = 0;
    for (size_t i = 0; i < xs_len; i++) {
        long long c = 0;
        for (size_t j = 0; j < xs_len; j++) if (xs[j] == xs[i]) c++;
        if (c > best_count || (c == best_count && xs[i] < best)) {
            best = xs[i];
            best_count = c;
        }
    }
    *out = best;
    return 0;
}
