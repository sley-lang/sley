#include <stddef.h>
#include <stdlib.h>
#include <limits.h>

static int pc_cmp_ll(const void *pa, const void *pb) {
    long long a = *(const long long *)pa;
    long long b = *(const long long *)pb;
    return (a > b) - (a < b);
}

/* Returns sign of (a + b - k) computed exactly, without overflow. */
static int pc_sum_cmp(long long a, long long b, long long k) {
    /* a + b exceeds LLONG_MAX: certainly greater than k */
    if (b > 0 && a > LLONG_MAX - b) return 1;
    /* a + b below LLONG_MIN: certainly less than k */
    if (b < 0 && a < LLONG_MIN - b) return -1;
    long long s = a + b;
    return (s > k) - (s < k);
}

long long pair_count(const long long *xs, size_t xs_len, long long k) {
    if (xs == NULL || xs_len < 2) return 0;

    long long *a = malloc(xs_len * sizeof *a);
    if (a == NULL) {
        /* Fallback: quadratic count, exact arithmetic. */
        long long cnt = 0;
        for (size_t i = 0; i < xs_len; i++)
            for (size_t j = i + 1; j < xs_len; j++)
                if (pc_sum_cmp(xs[i], xs[j], k) == 0) cnt++;
        return cnt;
    }

    for (size_t i = 0; i < xs_len; i++) a[i] = xs[i];
    qsort(a, xs_len, sizeof *a, pc_cmp_ll);

    long long cnt = 0;
    size_t i = 0, j = xs_len - 1;
    while (i < j) {
        int s = pc_sum_cmp(a[i], a[j], k);
        if (s < 0) {
            i++;
        } else if (s > 0) {
            j--;
        } else {
            if (a[i] == a[j]) {
                long long m = (long long)(j - i + 1);
                cnt += m * (m - 1) / 2;
                break;
            }
            size_t ci = 1;
            while (i + ci < j && a[i + ci] == a[i]) ci++;
            size_t cj = 1;
            while (j - cj > i && a[j - cj] == a[j]) cj++;
            cnt += (long long)ci * (long long)cj;
            i += ci;
            j -= cj;
        }
    }

    free(a);
    return cnt;
}
