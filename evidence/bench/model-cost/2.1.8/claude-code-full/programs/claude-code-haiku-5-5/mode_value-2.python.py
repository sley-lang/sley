from collections import Counter


def mode_value(xs: list[int]) -> int:
    """Return the most frequent value in xs; ties go to the smallest value.

    Raises ValueError if xs is empty.
    """
    if not xs:
        raise ValueError("mode_value() arg is an empty list")
    counts = Counter(xs)
    best_count = max(counts.values())
    return min(v for v, c in counts.items() if c == best_count)
