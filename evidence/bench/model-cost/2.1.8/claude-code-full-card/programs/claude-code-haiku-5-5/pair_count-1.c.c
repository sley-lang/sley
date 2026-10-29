#include <stddef.h>
#include <stdlib.h>
#include <string.h>

static int ll_cmp(const void *pa, const void *pb) {
    long long a = *(const long long *)pa;
    long long b = *(const long long *)pb;
    return (a > b) - (a < b);
}

long long pair_count(const long long *xs, size_t xs_len, long long k) {
    if (xs == NULL || xs_len < 2) {
        return 0;
    }

    long long *sorted = malloc(xs_len * sizeof *sorted);
    if (sorted == NULL) {
        return 0;
    }
    memcpy(sorted, xs, xs_len * sizeof *sorted);
    qsort(sorted, xs_len, sizeof *sorted, ll_cmp);

    long long total = 0;
    size_t lo = 0;
    size_t hi = xs_len - 1;

    while (lo < hi) {
        /* Use 128-bit sum so a + b cannot overflow. */
        __int128 s = (__int128)sorted[lo] + (__int128)sorted[hi];
        __int128 target = (__int128)k;

        if (s < target) {
            lo++;
        } else if (s > target) {
            hi--;
        } else if (sorted[lo] == sorted[hi]) {
            /* All elements in [lo, hi] are equal; choose any two. */
            long long m = (long long)(hi - lo + 1);
            total += m * (m - 1) / 2;
            break;
        } else {
            /* Count the run of equal values at each end. */
            long long left_val = sorted[lo];
            long long right_val = sorted[hi];
            long long a = 0;
            long long b = 0;
            while (lo + (size_t)a <= hi && sorted[lo + (size_t)a] == left_val) {
                a++;
            }
            while (hi >= lo + (size_t)b + 1 && sorted[hi - (size_t)b] == right_val) {
                b++;
            }
            total += a * b;
            lo += (size_t)a;
            hi -= (size_t)b;
        }
    }

    free(sorted);
    return total;
}
