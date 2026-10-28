long long tak(long long x, long long y, long long z) {
    if (y < x)
        return tak(tak(x - 1, y, z), tak(y - 1, z, x), tak(z - 1, x, y));
    return z;
}
