#include <stddef.h>
#include <stdlib.h>

static int cmp_ll(const void *pa, const void *pb) {
    long long a = *(const long long *)pa;
    long long b = *(const long long *)pb;
    return (a > b) - (a < b);
}

int mode_value(const long long *xs, size_t xs_len, long long *out) {
    if (xs == NULL || xs_len == 0 || out == NULL) {
        return 1;
    }

    long long *tmp = malloc(xs_len * sizeof *tmp);
    if (tmp == NULL) {
        return 1;
    }

    for (size_t i = 0; i < xs_len; i++) {
        tmp[i] = xs[i];
    }
    qsort(tmp, xs_len, sizeof *tmp, cmp_ll);

    long long best_val = tmp[0];
    size_t best_cnt = 0;

    size_t i = 0;
    while (i < xs_len) {
        size_t j = i;
        while (j < xs_len && tmp[j] == tmp[i]) {
            j++;
        }
        size_t cnt = j - i;
        /* Sorted ascending: a strictly larger count is needed to replace,
           so ties keep the smaller value already recorded. */
        if (cnt > best_cnt) {
            best_cnt = cnt;
            best_val = tmp[i];
        }
        i = j;
    }

    free(tmp);
    *out = best_val;
    return 0;
}
