#include <stddef.h>

long long longest_run(const long long *xs, size_t xs_len) {
    if (xs == NULL || xs_len == 0) {
        return 0;
    }

    size_t best = 1;
    size_t current = 1;

    for (size_t i = 1; i < xs_len; i++) {
        if (xs[i] == xs[i - 1]) {
            current++;
        } else {
            current = 1;
        }
        if (current > best) {
            best = current;
        }
    }

    return (long long)best;
}
