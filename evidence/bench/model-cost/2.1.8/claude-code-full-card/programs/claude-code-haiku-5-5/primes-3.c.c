long long primes(long long n)
{
    if (n <= 2) {
        return 0;
    }

    unsigned char *composite = (unsigned char *)calloc((size_t)n, sizeof(unsigned char));
    if (composite == NULL) {
        return 0;
    }

    long long count = 0;
    for (long long i = 2; i < n; i++) {
        if (!composite[i]) {
            count++;
            for (long long j = i * i; j < n; j += i) {
                composite[j] = 1;
            }
        }
    }

    free(composite);
    return count;
}
