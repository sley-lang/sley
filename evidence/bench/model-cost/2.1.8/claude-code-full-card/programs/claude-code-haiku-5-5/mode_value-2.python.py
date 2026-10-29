from collections import Counter


def mode_value(xs: list[int]) -> int:
    if not xs:
        raise ValueError("mode_value: empty input")
    counts = Counter(xs)
    best_count = max(counts.values())
    return min(v for v, c in counts.items() if c == best_count)
