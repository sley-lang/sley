def inversions(xs: list[int]) -> int:
    count: int = 0
    n: int = len(xs)
    for i in range(n):
        for j in range(i + 1, n):
            if xs[i] > xs[j]:
                count += 1
    return count
