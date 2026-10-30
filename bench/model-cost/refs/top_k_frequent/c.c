#include <stddef.h>

void top_k_frequent(const long long *xs, size_t xs_len, long long k, long long *out, size_t *out_len)
{
    long long vals[200];
    long long counts[200];
    size_t n = 0;

    for (size_t i = 0; i < xs_len; i++) {
        size_t j;
        for (j = 0; j < n; j++) {
            if (vals[j] == xs[i])
                break;
        }
        if (j == n) {
            vals[n] = xs[i];
            counts[n] = 0;
            n++;
        }
        counts[j]++;
    }

    /* insertion sort: descending count, then ascending value */
    for (size_t i = 1; i < n; i++) {
        long long v = vals[i];
        long long c = counts[i];
        size_t j = i;
        while (j > 0 && (counts[j - 1] < c || (counts[j - 1] == c && vals[j - 1] > v))) {
            vals[j] = vals[j - 1];
            counts[j] = counts[j - 1];
            j--;
        }
        vals[j] = v;
        counts[j] = c;
    }

    size_t m = n;
    if (k < 0)
        m = 0;
    else if ((unsigned long long)k < (unsigned long long)n)
        m = (size_t)k;
    for (size_t i = 0; i < m; i++)
        out[i] = vals[i];
    *out_len = m;
}
