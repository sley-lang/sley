#include <stddef.h>

int covered_length(const long long *xs, size_t xs_len, long long *out)
{
    long long st[100];
    long long en[100];
    size_t n;
    long long total = 0;

    if (xs_len % 2 != 0)
        return 1;
    n = xs_len / 2;
    for (size_t i = 0; i < n; i++) {
        st[i] = xs[2 * i];
        en[i] = xs[2 * i + 1];
        if (st[i] > en[i])
            return 1;
    }

    /* insertion sort by start */
    for (size_t i = 1; i < n; i++) {
        long long a = st[i];
        long long b = en[i];
        size_t j = i;
        while (j > 0 && st[j - 1] > a) {
            st[j] = st[j - 1];
            en[j] = en[j - 1];
            j--;
        }
        st[j] = a;
        en[j] = b;
    }

    if (n > 0) {
        long long cs = st[0];
        long long ce = en[0];
        for (size_t i = 1; i < n; i++) {
            if (st[i] <= ce) {
                if (en[i] > ce)
                    ce = en[i];
            } else {
                total += ce - cs;
                cs = st[i];
                ce = en[i];
            }
        }
        total += ce - cs;
    }
    *out = total;
    return 0;
}
