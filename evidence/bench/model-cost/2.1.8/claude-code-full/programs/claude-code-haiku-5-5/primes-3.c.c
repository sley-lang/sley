#include <stdbool.h>

long long primes(long long n) {
    if (n <= 2) {
        return 0;
    }

    long long count = 0;
    for (long long i = 2; i < n; i++) {
        bool is_prime = true;
        for (long long d = 2; d * d <= i; d++) {
            if (i % d == 0) {
                is_prime = false;
                break;
            }
        }
        if (is_prime) {
            count++;
        }
    }
    return count;
}
