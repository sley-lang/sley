#include <stddef.h>

int second_largest(const long long *xs, size_t xs_len, long long *out) {
    if (xs_len == 0) return 1;
    long long top = xs[0];
    for (size_t i = 1; i < xs_len; i++) if (xs[i] > top) top = xs[i];
    int found = 0;
    long long second = 0;
    for (size_t i = 0; i < xs_len; i++) {
        if (xs[i] != top && (!found || xs[i] > second)) {
            second = xs[i];
            found = 1;
        }
    }
    if (!found) return 1;
    *out = second;
    return 0;
}
