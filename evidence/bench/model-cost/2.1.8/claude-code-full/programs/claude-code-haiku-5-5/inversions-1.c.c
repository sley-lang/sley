#include <stddef.h>
#include <stdlib.h>

static long long merge_count(long long *a, long long *tmp, size_t lo, size_t hi) {
    if (hi - lo < 2) {
        return 0;
    }
    size_t mid = lo + (hi - lo) / 2;
    long long count = merge_count(a, tmp, lo, mid) + merge_count(a, tmp, mid, hi);

    size_t i = lo, j = mid, k = lo;
    while (i < mid && j < hi) {
        if (a[j] < a[i]) {
            /* a[j] is smaller than every remaining element of the left half */
            count += (long long)(mid - i);
            tmp[k++] = a[j++];
        } else {
            tmp[k++] = a[i++];
        }
    }
    while (i < mid) {
        tmp[k++] = a[i++];
    }
    while (j < hi) {
        tmp[k++] = a[j++];
    }
    for (size_t t = lo; t < hi; t++) {
        a[t] = tmp[t];
    }
    return count;
}

long long inversions(const long long *xs, size_t xs_len) {
    if (xs == NULL || xs_len < 2) {
        return 0;
    }

    long long *work = malloc(xs_len * sizeof *work);
    long long *tmp = malloc(xs_len * sizeof *tmp);
    if (work == NULL || tmp == NULL) {
        /* Fallback: quadratic count without auxiliary memory. */
        free(work);
        free(tmp);
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
        work[i] = xs[i];
    }

    long long result = merge_count(work, tmp, 0, xs_len);

    free(work);
    free(tmp);
    return result;
}
