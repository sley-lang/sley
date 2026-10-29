#include <stddef.h>
#include <stdlib.h>

static long long merge_count(long long *a, long long *tmp, size_t lo, size_t hi) {
    if (hi - lo < 2) {
        return 0;
    }
    size_t mid = lo + (hi - lo) / 2;
    long long inv = merge_count(a, tmp, lo, mid) + merge_count(a, tmp, mid, hi);

    size_t i = lo, j = mid, k = lo;
    while (i < mid && j < hi) {
        if (a[i] <= a[j]) {
            tmp[k++] = a[i++];
        } else {
            /* a[j] is smaller than every remaining left element */
            inv += (long long)(mid - i);
            tmp[k++] = a[j++];
        }
    }
    while (i < mid) {
        tmp[k++] = a[i++];
    }
    while (j < hi) {
        tmp[k++] = a[j++];
    }
    for (size_t p = lo; p < hi; p++) {
        a[p] = tmp[p];
    }
    return inv;
}

long long inversions(const long long *xs, size_t xs_len) {
    if (xs == NULL || xs_len < 2) {
        return 0;
    }

    long long *a = malloc(xs_len * sizeof *a);
    long long *tmp = malloc(xs_len * sizeof *tmp);
    if (a == NULL || tmp == NULL) {
        free(a);
        free(tmp);
        /* Fallback without extra memory: direct pair counting. */
        long long count = 0;
        for (size_t i = 0; i < xs_len; i++) {
            for (size_t j = i + 1; j < xs_len; j++) {
                if (xs[i] > xs[j]) {
                    count++;
                }
            }
        }
        return count;
    }

    for (size_t i = 0; i < xs_len; i++) {
        a[i] = xs[i];
    }
    long long result = merge_count(a, tmp, 0, xs_len);

    free(a);
    free(tmp);
    return result;
}
