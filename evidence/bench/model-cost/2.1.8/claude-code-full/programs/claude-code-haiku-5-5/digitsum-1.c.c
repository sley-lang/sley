#include <stddef.h>

long long digitsum(long long n)
{
    long long total = 0;

    for (long long i = 0; i < n; i++) {
        long long x = i;
        while (x > 0) {
            total += x % 10;
            x /= 10;
        }
    }

    return total;
}
