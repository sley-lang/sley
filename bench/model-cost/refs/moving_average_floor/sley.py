def moving_average_floor(xs: list[int], w: int) -> list[int]:
    if w < 1:
        raise ValueError("w must be at least 1")
    out = []
    for i in range(len(xs) - w + 1):
        out.append(sum(xs[i:i + w]) // w)
    return out
