#include <stdbool.h>

long long primes(long long n) {
    long long count = 0;
    for (long long k = 2; k < n; k++) {
        bool is_p = true;
        for (long long d = 2; d * d <= k; d++) {
            if (k % d == 0) {
                is_p = false;
                break;
            }
        }
        if (is_p)
            count++;
    }
    return count;
}
