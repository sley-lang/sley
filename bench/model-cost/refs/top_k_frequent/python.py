def top_k_frequent(xs: list[int], k: int) -> list[int]:
    counts = {}
    for x in xs:
        counts[x] = counts.get(x, 0) + 1
    ordered = sorted(counts, key=lambda v: (-counts[v], v))
    return ordered[:max(k, 0)]
