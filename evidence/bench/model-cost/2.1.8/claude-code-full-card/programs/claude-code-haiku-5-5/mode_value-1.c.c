#include <stdlib.h>
#include <stddef.h>

static int cmp_ll(const void *a, const void *b) {
    long long x = *(const long long *)a;
    long long y = *(const long long *)b;
    return (x > y) - (x < y);
}

int mode_value(const long long *xs, size_t xs_len, long long *out) {
    if (out == NULL || xs_len == 0 || xs == NULL) {
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
    size_t best_count = 0;

    size_t i = 0;
    while (i < xs_len) {
        size_t j = i;
        while (j < xs_len && tmp[j] == tmp[i]) {
            j++;
        }
        size_t count = j - i;
        /* Runs are visited in ascending value order, so a strictly
           greater count is required to replace the current best;
           ties keep the smaller value already recorded. */
        if (count > best_count) {
            best_count = count;
            best_val = tmp[i];
        }
        i = j;
    }

    free(tmp);
    *out = best_val;
    return 0;
}
