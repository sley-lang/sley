def dense_rank(xs: list[int]) -> list[int]:
    distinct = []
    for x in xs:
        if not any(d == x for d in distinct):
            distinct.append(x)
    return [1 + sum(1 for d in distinct if d > v) for v in xs]
