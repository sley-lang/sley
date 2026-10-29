from collections import Counter


def pair_count(xs: list[int], k: int) -> int:
    """Return the number of index pairs i < j with xs[i] + xs[j] == k."""
    seen = Counter()
    total = 0
    for x in xs:
        total += seen[k - x]
        seen[x] += 1
    return total
