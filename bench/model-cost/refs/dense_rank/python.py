def dense_rank(xs: list[int]) -> list[int]:
    ranks = {v: i + 1 for i, v in enumerate(sorted(set(xs), reverse=True))}
    return [ranks[x] for x in xs]
