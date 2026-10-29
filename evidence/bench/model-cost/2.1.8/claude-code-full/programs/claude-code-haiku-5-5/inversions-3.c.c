#include <stddef.h>
#include <stdlib.h>
#include <string.h>

/* Merge sorted runs a[lo..mid) and a[mid..hi) into a[lo..hi),
   counting pairs (i in left run, j in right run) with a[i] > a[j]. */
static long long merge_count(long long *a, long long *tmp,
                             size_t lo, size_t mid, size_t hi) {
    size_t i = lo, j = mid, k = lo;
    long long cnt = 0;

    while (i < mid && j < hi) {
        if (a[j] < a[i]) {
            /* a[j] is smaller than every remaining element of the left run */
            cnt += (long long)(mid - i);
            tmp[k++] = a[j++];
        } else {
            tmp[k++] = a[i++];
        }
    }
    while (i < mid) tmp[k++] = a[i++];
    while (j < hi)  tmp[k++] = a[j++];

    memcpy(a + lo, tmp + lo, (hi - lo) * sizeof *a);
    return cnt;
}

long long inversions(const long long *xs, size_t xs_len) {
    if (xs == NULL || xs_len < 2) {
        return 0;
    }

    size_t n = xs_len;
    long long *a = malloc(n * sizeof *a);
    long long *tmp = malloc(n * sizeof *tmp);

    if (a == NULL || tmp == NULL) {
        /* Allocation failed: fall back to the quadratic count, which needs no memory. */
        free(a);
        free(tmp);
        long long total = 0;
        for (size_t i = 0; i < n; i++) {
            for (size_t j = i + 1; j < n; j++) {
                if (xs[i] > xs[j]) total++;
            }
        }
        return total;
    }

    memcpy(a, xs, n * sizeof *a);

    long long total = 0;
    /* Bottom-up merge sort, counting inversions during each merge. */
    for (size_t width = 1; width < n; width *= 2) {
        for (size_t lo = 0; lo + width < n; lo += 2 * width) {
            size_t mid = lo + width;
            size_t hi = lo + 2 * width;
            if (hi > n) hi = n;
            total += merge_count(a, tmp, lo, mid, hi);
        }
    }

    free(a);
    free(tmp);
    return total;
}
