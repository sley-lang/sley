def pair_count(xs: list[int], k: int) -> int:
    n = len(xs)
    count = 0
    for i in range(n):
        for j in range(i + 1, n):
            if xs[i] + xs[j] == k:
                count += 1
    return count
