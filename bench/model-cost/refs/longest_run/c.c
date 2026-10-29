#include <stddef.h>

long long longest_run(const long long *xs, size_t xs_len) {
    long long best = 0, run = 0;
    for (size_t i = 0; i < xs_len; i++) {
        run = (i > 0 && xs[i] == xs[i - 1]) ? run + 1 : 1;
        if (run > best) best = run;
    }
    return best;
}
