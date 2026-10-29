#include <stddef.h>
#include <stdlib.h>

static long long count_and_sort(long long *a, long long *tmp, size_t n) {
    if (n < 2) {
        return 0;
    }
    size_t mid = n / 2;
    long long cnt = count_and_sort(a, tmp, mid) + count_and_sort(a + mid, tmp, n - mid);

    size_t i = 0, j = mid, k = 0;
    while (i < mid && j < n) {
        if (a[j] < a[i]) {
            /* a[j] is smaller than every remaining element of the left run */
            cnt += (long long)(mid - i);
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
    for (size_t t = 0; t < n; t++) {
        a[t] = tmp[t];
    }
    return cnt;
}

long long inversions(const long long *xs, size_t xs_len) {
    if (xs == NULL || xs_len < 2) {
        return 0;
    }

    long long *a = malloc(xs_len * sizeof *a);
    long long *tmp = malloc(xs_len * sizeof *tmp);
    if (a == NULL || tmp == NULL) {
        /* Fallback: quadratic count, needs no extra memory. */
        free(a);
        free(tmp);
        long long total = 0;
        for (size_t i = 0; i < xs_len; i++) {
            for (size_t j = i + 1; j < xs_len; j++) {
                if (xs[i] > xs[j]) {
                    total++;
                }
            }
        }
        return total;
    }

    for (size_t i = 0; i < xs_len; i++) {
        a[i] = xs[i];
    }
    long long result = count_and_sort(a, tmp, xs_len);

    free(a);
    free(tmp);
    return result;
}
