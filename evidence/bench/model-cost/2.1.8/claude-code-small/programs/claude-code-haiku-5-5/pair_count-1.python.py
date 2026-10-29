def pair_count(xs: list[int], k: int) -> int:
    seen: dict[int, int] = {}
    total = 0
    for x in xs:
        total += seen.get(k - x, 0)
        seen[x] = seen.get(x, 0) + 1
    return total
