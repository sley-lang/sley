#include <stddef.h>

long long transform(const long long *xs, size_t xs_len, long long lo, long long hi) {
    unsigned long long sum = 0;

    for (size_t i = 0; i < xs_len; i++) {
        long long x = xs[i];
        long long y;

        if (x % 2 != 0) {
            /* Odd: compute 2x, saturating to avoid signed overflow.
               Saturating is safe because the result is clamped afterward. */
            if (x > __LONG_LONG_MAX__ / 2) {
                y = hi;
            } else if (x < -(__LONG_LONG_MAX__ / 2) - 1) {
                y = lo;
            } else {
                y = 2 * x;
            }
        } else {
            y = x;
        }

        if (y < lo) {
            y = lo;
        } else if (y > hi) {
            y = hi;
        }

        /* Unsigned accumulation avoids signed overflow UB; the final
           conversion wraps on gcc (two's complement). */
        sum += (unsigned long long)y;
    }

    return (long long)sum;
}
