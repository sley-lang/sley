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

    long long *a = malloc(xs_len * sizeof *a);
    if (a == NULL) {
        /* Fallback without extra memory: direct O(n^2) scan. */
        long long total = 0;
        for (size_t i = 0; i < xs_len; i++) {
            for (size_t j = i + 1; j < xs_len; j++) {
                if ((__int128)xs[i] + (__int128)xs[j] == (__int128)k) {
                    total++;
                }
            }
        }
        return total;
    }
    memcpy(a, xs, xs_len * sizeof *a);
    qsort(a, xs_len, sizeof *a, ll_cmp);

    long long count = 0;
    size_t lo = 0;
    size_t hi = xs_len - 1;

    while (lo < hi) {
        __int128 s = (__int128)a[lo] + (__int128)a[hi];
        __int128 target = (__int128)k;

        if (s < target) {
            lo++;
        } else if (s > target) {
            hi--;
        } else {
            if (a[lo] == a[hi]) {
                /* All remaining elements in [lo, hi] are equal. */
                long long m = (long long)(hi - lo + 1);
                count += m * (m - 1) / 2;
                break;
            }

            long long vl = a[lo];
            long long vh = a[hi];
            long long cl = 0;
            long long ch = 0;

            while (lo <= hi && a[lo] == vl) {
                cl++;
                lo++;
            }
            while (hi >= lo && a[hi] == vh) {
                ch++;
                if (hi == 0) {
                    break;
                }
                hi--;
            }

            count += cl * ch;
        }
    }

    free(a);
    return count;
}
