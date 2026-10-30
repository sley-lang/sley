def pair_count(xs: list[int], k: int) -> int:
    counts = {}
    total = 0
    for x in xs:
        total += counts.get(k - x, 0)
        counts[x] = counts.get(x, 0) + 1
    return total
