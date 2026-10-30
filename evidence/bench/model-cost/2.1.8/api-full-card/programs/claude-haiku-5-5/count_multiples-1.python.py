def count_multiples(xs: list[int], k: int) -> int:
    if not isinstance(k, int) or isinstance(k, bool) or k <= 0:
        raise ValueError("k must be a positive integer")
    return sum(1 for x in xs if x % k == 0)
