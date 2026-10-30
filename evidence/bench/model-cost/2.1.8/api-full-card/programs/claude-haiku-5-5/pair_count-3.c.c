#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <stddef.h>

static int ll_cmp_for_sort(const void *pa, const void *pb) {
    long long a = *(const long long *)pa;
    long long b = *(const long long *)pb;
    return (a > b) - (a < b);
}

/* Returns sign of (a + b - k) without signed overflow. */
static int sum_sign(long long a, long long b, long long k) {
    long long s;
    if (b > 0 && a > LLONG_MAX - b) return 1;   /* a+b > LLONG_MAX >= k */
    if (b < 0 && a < LLONG_MIN - b) return -1;  /* a+b < LLONG_MIN <= k */
    s = a + b;
    return (s > k) - (s < k);
}

long long pair_count(const long long *xs, size_t xs_len, long long k) {
    if (xs == NULL || xs_len < 2) return 0;

    long long *v = malloc(xs_len * sizeof *v);
    if (v == NULL) return 0;
    memcpy(v, xs, xs_len * sizeof *v);
    qsort(v, xs_len, sizeof *v, ll_cmp_for_sort);

    long long cnt = 0;
    size_t lo = 0, hi = xs_len - 1;

    while (lo < hi) {
        int s = sum_sign(v[lo], v[hi], k);
        if (s < 0) {
            lo++;
        } else if (s > 0) {
            hi--;
        } else {
            if (v[lo] == v[hi]) {
                long long m = (long long)(hi - lo + 1);
                cnt += m * (m - 1) / 2;
                break;
            }
            long long ca = 0, cb = 0;
            long long a = v[lo], b = v[hi];
            while (lo <= hi && v[lo] == a) { ca++; lo++; }
            while (hi >= lo && v[hi] == b) { cb++; if (hi == 0) break; hi--; }
            cnt += ca * cb;
        }
    }

    free(v);
    return cnt;
}
