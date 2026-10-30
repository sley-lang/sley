#include <stdlib.h>
#include <string.h>
#include <stddef.h>

static int pc_cmp_ll(const void *a, const void *b) {
    long long x = *(const long long *)a;
    long long y = *(const long long *)b;
    return (x > y) - (x < y);
}

long long pair_count(const long long *xs, size_t xs_len, long long k) {
    if (xs == NULL || xs_len < 2) {
        return 0;
    }

    size_t n = xs_len;
    long long *a = malloc(n * sizeof *a);
    if (a == NULL) {
        /* Fallback: quadratic scan, overflow-safe via __int128 */
        long long cnt = 0;
        for (size_t i = 0; i < n; i++) {
            for (size_t j = i + 1; j < n; j++) {
                __int128 s = (__int128)xs[i] + (__int128)xs[j];
                if (s == (__int128)k) {
                    cnt++;
                }
            }
        }
        return cnt;
    }

    memcpy(a, xs, n * sizeof *a);
    qsort(a, n, sizeof *a, pc_cmp_ll);

    long long cnt = 0;
    size_t l = 0;
    size_t r = n - 1;

    while (l < r) {
        __int128 s = (__int128)a[l] + (__int128)a[r];
        if (s < (__int128)k) {
            l++;
        } else if (s > (__int128)k) {
            r--;
        } else {
            if (a[l] == a[r]) {
                long long m = (long long)(r - l + 1);
                cnt += m * (m - 1) / 2;
                break;
            }
            long long vl = a[l];
            long long vr = a[r];
            long long cl = 0;
            long long cr = 0;
            while (l <= r && a[l] == vl) {
                cl++;
                l++;
            }
            while (r >= l && r < n && a[r] == vr) {
                cr++;
                if (r == 0) {
                    break;
                }
                r--;
            }
            cnt += cl * cr;
        }
    }

    free(a);
    return cnt;
}
