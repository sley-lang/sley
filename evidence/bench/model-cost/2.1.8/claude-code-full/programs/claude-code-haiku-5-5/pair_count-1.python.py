from collections import Counter


def pair_count(xs: list[int], k: int) -> int:
    seen = Counter()
    total = 0
    for x in xs:
        total += seen[k - x]
        seen[x] += 1
    return total
