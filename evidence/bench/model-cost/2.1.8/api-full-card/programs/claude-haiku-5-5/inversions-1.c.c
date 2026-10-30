#include <stddef.h>
#include <stdlib.h>

long long inversions(const long long *xs, size_t xs_len) {
    if (xs == NULL || xs_len < 2) {
        return 0;
    }

    size_t n = xs_len;
    long long *a = malloc(n * sizeof *a);
    long long *tmp = malloc(n * sizeof *tmp);

    if (a == NULL || tmp == NULL) {
        /* Fallback: quadratic count, no extra memory needed. */
        free(a);
        free(tmp);
        long long count = 0;
        for (size_t i = 0; i < n; i++) {
            for (size_t j = i + 1; j < n; j++) {
                if (xs[i] > xs[j]) {
                    count++;
                }
            }
        }
        return count;
    }

    for (size_t i = 0; i < n; i++) {
        a[i] = xs[i];
    }

    long long count = 0;
    for (size_t width = 1; width < n; width *= 2) {
        for (size_t lo = 0; lo < n; lo += 2 * width) {
            size_t mid = lo + width;
            if (mid > n) {
                mid = n;
            }
            size_t hi = lo + 2 * width;
            if (hi > n) {
                hi = n;
            }

            size_t i = lo, j = mid, k = lo;
            while (i < mid && j < hi) {
                if (a[i] <= a[j]) {
                    tmp[k++] = a[i++];
                } else {
                    count += (long long)(mid - i);
                    tmp[k++] = a[j++];
                }
            }
            while (i < mid) {
                tmp[k++] = a[i++];
            }
            while (j < hi) {
                tmp[k++] = a[j++];
            }
        }
        long long *swap = a;
        a = tmp;
        tmp = swap;
    }

    free(a);
    free(tmp);
    return count;
}
