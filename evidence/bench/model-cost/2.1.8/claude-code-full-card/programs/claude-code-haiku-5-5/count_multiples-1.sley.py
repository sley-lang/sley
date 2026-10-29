def count_multiples(xs: list[int], k: int) -> int:
    if k <= 0:
        raise ValueError("k must be positive")
    return sum(1 for x in xs if x % k == 0)
