from collections import defaultdict


def pair_count(xs: list[int], k: int) -> int:
    seen = defaultdict(int)
    total = 0
    for x in xs:
        total += seen[k - x]
        seen[x] += 1
    return total
