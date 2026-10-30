def max_depth(codes: list[int]) -> int:
    depth = 0
    best = 0
    for c in codes:
        if c == 40:
            depth += 1
            if depth > best:
                best = depth
        elif c == 41:
            if depth == 0:
                raise ValueError("unbalanced: close without open")
            depth -= 1
    if depth != 0:
        raise ValueError("unbalanced: open never closed")
    return best
