#include <stdlib.h>
#include <string.h>

static int cmp_ll(const void *a, const void *b) {
    long long x = *(const long long *)a;
    long long y = *(const long long *)b;
    return (x > y) - (x < y);
}

int mode_value(const long long *xs, size_t xs_len, long long *out) {
    if (xs == NULL || xs_len == 0 || out == NULL) {
        return 1;
    }

    long long *tmp = malloc(xs_len * sizeof *tmp);
    if (tmp == NULL) {
        return 1;
    }
    memcpy(tmp, xs, xs_len * sizeof *tmp);
    qsort(tmp, xs_len, sizeof *tmp, cmp_ll);

    long long best = tmp[0];
    size_t best_count = 0;
    size_t i = 0;
    while (i < xs_len) {
        size_t j = i;
        while (j < xs_len && tmp[j] == tmp[i]) {
            j++;
        }
        size_t count = j - i;
        if (count > best_count) {
            best_count = count;
            best = tmp[i];
        }
        i = j;
    }

    free(tmp);
    *out = best;
    return 0;
}
