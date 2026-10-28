def inversions(xs: list[int]) -> int:
    count = 0
    n = len(xs)
    for i in range(n):
        x = xs[i]
        for j in range(i + 1, n):
            if xs[j] < x:
                count += 1
    return count
