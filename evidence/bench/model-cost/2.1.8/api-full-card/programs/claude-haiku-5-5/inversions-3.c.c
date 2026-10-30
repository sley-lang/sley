#include <stddef.h>
#include <stdlib.h>
#include <string.h>

static long long merge_count(long long *a, long long *tmp, size_t lo, size_t hi)
{
    /* sorts a[lo..hi) and returns inversions within that range */
    long long count = 0;
    if (hi - lo < 2)
        return 0;

    size_t mid = lo + (hi - lo) / 2;
    count += merge_count(a, tmp, lo, mid);
    count += merge_count(a, tmp, mid, hi);

    size_t i = lo, j = mid, k = lo;
    while (i < mid && j < hi) {
        if (a[i] <= a[j]) {
            tmp[k++] = a[i++];
        } else {
            /* a[j] is smaller than all remaining left elements */
            count += (long long)(mid - i);
            tmp[k++] = a[j++];
        }
    }
    while (i < mid)
        tmp[k++] = a[i++];
    while (j < hi)
        tmp[k++] = a[j++];

    memcpy(a + lo, tmp + lo, (hi - lo) * sizeof(long long));
    return count;
}

long long inversions(const long long *xs, size_t xs_len)
{
    if (xs == NULL || xs_len < 2)
        return 0;

    long long *a = malloc(xs_len * sizeof(long long));
    long long *tmp = malloc(xs_len * sizeof(long long));
    if (a == NULL || tmp == NULL) {
        free(a);
        free(tmp);
        return 0;
    }

    memcpy(a, xs, xs_len * sizeof(long long));
    long long result = merge_count(a, tmp, 0, xs_len);

    free(a);
    free(tmp);
    return result;
}
