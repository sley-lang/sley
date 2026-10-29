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

    long long *copy = malloc(xs_len * sizeof *copy);
    if (copy == NULL) {
        return 1;
    }

    for (size_t i = 0; i < xs_len; i++) {
        copy[i] = xs[i];
    }
    qsort(copy, xs_len, sizeof *copy, cmp_ll);

    long long best_val = copy[0];
    size_t best_count = 0;

    size_t i = 0;
    while (i < xs_len) {
        size_t j = i;
        while (j < xs_len && copy[j] == copy[i]) {
            j++;
        }
        size_t run = j - i;
        /* Sorted ascending, so strict '>' keeps the smallest value on ties. */
        if (run > best_count) {
            best_count = run;
            best_val = copy[i];
        }
        i = j;
    }

    free(copy);
    *out = best_val;
    return 0;
}
