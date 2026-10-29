#include <stddef.h>
#include <stdlib.h>

static int cmp_ll(const void *p, const void *q) {
    long long x = *(const long long *)p;
    long long y = *(const long long *)q;
    return (x > y) - (x < y);
}

/* Count pairs i < j with xs[i] + xs[j] == k. Sorts a copy, then two-pointer scan.
   Overflow-safe comparison of a + b against k. */
static int cmp_sum(long long a, long long b, long long k) {
    if (b > 0 && a > (long long)(((unsigned long long)-1 >> 1)) - b) return 1;
    if (b < 0 && a < (-(long long)(((unsigned long long)-1 >> 1)) - 1) - b) return -1;
    long long s = a + b;
    return (s > k) - (s < k);
}

long long pair_count(const long long *xs, size_t xs_len, long long k) {
    if (xs == NULL || xs_len < 2) {
        return 0;
    }

    long long *a = malloc(xs_len * sizeof *a);
    if (a == NULL) {
        long long cnt = 0;
        for (size_t i = 0; i < xs_len; i++)
            for (size_t j = i + 1; j < xs_len; j++)
                if (cmp_sum(xs[i], xs[j], k) == 0)
                    cnt++;
        return cnt;
    }
    for (size_t i = 0; i < xs_len; i++) {
        a[i] = xs[i];
    }
    qsort(a, xs_len, sizeof *a, cmp_ll);

    long long ans = 0;
    size_t i = 0;
    size_t j = xs_len - 1;

    while (i < j) {
        int c = cmp_sum(a[i], a[j], k);
        if (c < 0) {
            i++;
        } else if (c > 0) {
            j--;
        } else {
            if (a[i] == a[j]) {
                long long m = (long long)(j - i + 1);
                ans += m * (m - 1) / 2;
                break;
            }
            size_t ei = i;
            while (ei <= j && a[ei] == a[i]) {
                ei++;
            }
            long long cl = (long long)(ei - i);

            size_t sj = j;
            while (sj > i && a[sj - 1] == a[j]) {
                sj--;
            }
            long long cr = (long long)(j - sj + 1);

            ans += cl * cr;
            i = ei;
            j = sj - 1;
        }
    }

    free(a);
    return ans;
}
