#include <stddef.h>

long long inversions(const long long *xs, long long n) {
    long long count = 0;
    for (long long i = 0; i < n; i++) {
        long long x = xs[i];
        for (long long j = i + 1; j < n; j++)
            if (xs[j] < x)
                count++;
    }
    return count;
}
