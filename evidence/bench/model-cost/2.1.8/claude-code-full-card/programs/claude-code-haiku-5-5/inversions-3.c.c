#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

/* Sorts a[0..n) in place using tmp as scratch space and returns the number
 * of pairs i < j (within this range) with a[i] > a[j]. */
static long long msort_count(long long *a, long long *tmp, size_t n) {
    if (n < 2) {
        return 0;
    }
    size_t mid = n / 2;
    long long cnt = msort_count(a, tmp, mid) + msort_count(a + mid, tmp, n - mid);

    size_t i = 0, j = mid, k = 0;
    while (i < mid && j < n) {
        if (a[j] < a[i]) {
            /* a[j] is smaller than every remaining element of the left run. */
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
    memcpy(a, tmp, n * sizeof *a);
    return cnt;
}

long long inversions(const long long *xs, size_t xs_len) {
    if (xs == NULL || xs_len < 2) {
        return 0;
    }

    /* Guard against overflow when sizing the buffers. */
    if (xs_len <= SIZE_MAX / (2 * sizeof(long long))) {
        long long *buf = malloc(2 * xs_len * sizeof *buf);
        if (buf != NULL) {
            long long *a = buf;
            long long *tmp = buf + xs_len;
            memcpy(a, xs, xs_len * sizeof *a);
            long long cnt = msort_count(a, tmp, xs_len);
            free(buf);
            return cnt;
        }
    }

    /* Fallback without extra memory: quadratic count on the original input. */
    long long cnt = 0;
    for (size_t i = 0; i < xs_len; i++) {
        for (size_t j = i + 1; j < xs_len; j++) {
            if (xs[i] > xs[j]) {
                cnt++;
            }
        }
    }
    return cnt;
}
