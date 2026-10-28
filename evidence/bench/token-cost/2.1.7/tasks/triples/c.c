long long triples(long long n) {
    long long count = 0;
    for (long long c = 1; c <= n; c++)
        for (long long b = 1; b < c; b++)
            for (long long a = 1; a < b; a++)
                if (a * a + b * b == c * c)
                    count++;
    return count;
}
