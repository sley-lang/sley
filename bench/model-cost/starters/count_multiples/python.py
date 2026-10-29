def count_multiples(xs: list[int], k: int) -> int:
    return sum(1 for x in xs if x % 2 == 0)
