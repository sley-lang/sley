def second_largest(xs: list[int]) -> int:
    if not isinstance(xs, list):
        raise ValueError("xs must be a list of integers")
    distinct = set()
    for x in xs:
        if isinstance(x, bool) or not isinstance(x, int):
            raise ValueError("all elements must be integers")
        distinct.add(x)
    if len(distinct) < 2:
        raise ValueError("need at least two distinct values")
    largest = max(distinct)
    distinct.remove(largest)
    return max(distinct)
