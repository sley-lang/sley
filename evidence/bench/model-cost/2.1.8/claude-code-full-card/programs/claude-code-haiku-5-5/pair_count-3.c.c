#include <limits.h>
#include <stddef.h>

/* Returns 1 if a + b == k exactly, 0 otherwise. Avoids signed overflow:
   if a + b is not representable as long long, it cannot equal k. */
static int sum_equals(long long a, long long b, long long k) {
    if (b > 0 && a > LLONG_MAX - b) return 0;
    if (b < 0 && a < LLONG_MIN - b) return 0;
    return a + b == k;
}

long long pair_count(const long long *xs, size_t xs_len, long long k) {
    long long count = 0;
    for (size_t i = 0; i < xs_len; i++) {
        for (size_t j = i + 1; j < xs_len; j++) {
            if (sum_equals(xs[i], xs[j], k)) {
                count++;
            }
        }
    }
    return count;
}
