#include <stddef.h>
#include <stdlib.h>

static int cmp_ll(const void *a, const void *b) {
    long long x = *(const long long *)a;
    long long y = *(const long long *)b;
    return (x > y) - (x < y);
}

int mode_value(const long long *xs, size_t xs_len, long long *out) {
    if (out == NULL || xs == NULL || xs_len == 0) {
        return 1;
    }

    long long *buf = malloc(xs_len * sizeof *buf);
    if (buf == NULL) {
        return 1;
    }

    for (size_t i = 0; i < xs_len; i++) {
        buf[i] = xs[i];
    }
    qsort(buf, xs_len, sizeof *buf, cmp_ll);

    long long best = buf[0];
    size_t best_count = 0;

    size_t i = 0;
    while (i < xs_len) {
        size_t j = i;
        while (j < xs_len && buf[j] == buf[i]) {
            j++;
        }
        size_t count = j - i;
        if (count > best_count) {
            best_count = count;
            best = buf[i];
        }
        i = j;
    }

    free(buf);
    *out = best;
    return 0;
}
