def pair_count(xs: list[int], k: int) -> int:
    count = 0
    n = len(xs)
    for i in range(n):
        for j in range(i + 1, n):
            if xs[i] + xs[j] == k:
                count += 1
    return count
