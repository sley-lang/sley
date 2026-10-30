#include <stddef.h>

long long first_overdraft(long long start, const long long *xs, size_t xs_len)
{
    long long balance = start;

    for (size_t i = 0; i < xs_len; i++) {
        balance += xs[i];
        if (balance < 0)
            return (long long)i;
    }
    return -1;
}
