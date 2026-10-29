#include <stddef.h>
#include <stdlib.h>
#include <string.h>

/* Counts inversions in a[0..n) while sorting it, using tmp as scratch space.
   An inversion is a pair i < j with a[i] > a[j]; equal elements are not counted. */
static long long merge_count(long long *a, long long *tmp, size_t n) {
    if (n < 2) {
        return 0;
    }
    size_t mid = n / 2;
    long long inv = merge_count(a, tmp, mid) + merge_count(a + mid, tmp, n - mid);

    size_t i = 0, j = mid, k = 0;
    while (i < mid && j < n) {
        if (a[j] < a[i]) {
            /* a[j] is smaller than every remaining element of the left half. */
            inv += (long long)(mid - i);
            tmp[k++] = a[j++];
        } else {
            tmp[k++] = a[i++];
        }
    }
    while (i < mid) {
        tmp[k++] = a[i++];
    }
    while (j < n) {
        tmp[k++] = a[j++];
    }
    memcpy(a, tmp, n * sizeof *a);
    return inv;
}

long long inversions(const long long *xs, size_t xs_len) {
    if (xs == NULL || xs_len < 2) {
        return 0;
    }

    long long *work = malloc(xs_len * sizeof *work);
    long long *tmp = malloc(xs_len * sizeof *tmp);
    if (work == NULL || tmp == NULL) {
        free(work);
        free(tmp);
        /* Fallback when memory is unavailable: quadratic scan. */
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

    memcpy(work, xs, xs_len * sizeof *work);
    long long result = merge_count(work, tmp, xs_len);

    free(work);
    free(tmp);
    return result;
}
