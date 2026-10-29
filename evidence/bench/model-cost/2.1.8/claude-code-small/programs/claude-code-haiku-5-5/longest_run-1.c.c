#include <stddef.h>

long long longest_run(const long long *xs, size_t xs_len) {
    if (xs_len == 0 || xs == NULL) {
        return 0;
    }

    size_t best = 1;
    size_t current = 1;

    for (size_t i = 1; i < xs_len; i++) {
        if (xs[i] == xs[i - 1]) {
            current++;
            if (current > best) {
                best = current;
            }
        } else {
            current = 1;
        }
    }

    return (long long)best;
}
