def pair_count(xs: list[int], k: int) -> int:
    total = 0
    n = len(xs)
    for i in range(n):
        for j in range(i + 1, n):
            if xs[i] + xs[j] == k:
                total += 1
    return total
