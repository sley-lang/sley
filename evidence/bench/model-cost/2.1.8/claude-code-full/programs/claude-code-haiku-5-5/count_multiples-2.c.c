#include <stddef.h>

int count_multiples(const long long *xs, size_t xs_len, long long k, long long *out) {
    if (k <= 0 || out == NULL) {
        return 1;
    }
    long long count = 0;
    for (size_t i = 0; i < xs_len; i++) {
        if (xs[i] % k == 0) {
            count++;
        }
    }
    *out = count;
    return 0;
}
