def max_depth(codes: list[int]) -> int:
    depth = 0
    max_d = 0
    for c in codes:
        if c == 40:
            depth += 1
            if depth > max_d:
                max_d = depth
        elif c == 41:
            depth -= 1
            if depth < 0:
                raise ValueError("unbalanced parentheses: close without open")
    if depth != 0:
        raise ValueError("unbalanced parentheses: open never closed")
    return max_d
