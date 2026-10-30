def count_by_remainder(xs: list[int], m: int) -> list[int]:
    if m < 1 or m > 200:
        raise ValueError("m out of range")
    counts = [0] * m
    for x in xs:
        counts[x % m] += 1
    return counts
