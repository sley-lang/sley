#include <stddef.h>
#include <stdlib.h>
#include <string.h>

/* Counts pairs i < j with xs[i] > xs[j] using a bottom-up merge sort on a copy.
 * The input array is not modified. Runs in O(n log n) time, O(n) extra space.
 * If the scratch allocation fails, falls back to an O(n^2) pair count. */
long long inversions(const long long *xs, size_t xs_len) {
    if (xs == NULL || xs_len < 2) {
        return 0;
    }

    long long *a = malloc(xs_len * sizeof *a);
    long long *b = malloc(xs_len * sizeof *b);
    if (a == NULL || b == NULL) {
        free(a);
        free(b);
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

    memcpy(a, xs, xs_len * sizeof *a);

    long long count = 0;
    for (size_t width = 1; width < xs_len; width *= 2) {
        for (size_t lo = 0; lo < xs_len; lo += 2 * width) {
            size_t mid = lo + width;
            if (mid > xs_len) {
                mid = xs_len;
            }
            size_t hi = lo + 2 * width;
            if (hi > xs_len) {
                hi = xs_len;
            }

            size_t i = lo, j = mid, k = lo;
            while (i < mid && j < hi) {
                if (a[i] <= a[j]) {
                    b[k++] = a[i++];
                } else {
                    /* a[j] is smaller than all remaining left elements. */
                    count += (long long)(mid - i);
                    b[k++] = a[j++];
                }
            }
            while (i < mid) {
                b[k++] = a[i++];
            }
            while (j < hi) {
                b[k++] = a[j++];
            }
        }

        long long *tmp = a;
        a = b;
        b = tmp;
    }

    free(a);
    free(b);
    return count;
}
