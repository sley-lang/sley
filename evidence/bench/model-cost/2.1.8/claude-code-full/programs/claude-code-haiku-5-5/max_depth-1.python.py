def max_depth(codes: list[int]) -> int:
    """Return the maximum nesting depth of parentheses in a list of character codes.

    Code 40 is '(' and code 41 is ')'. All other codes are ignored.
    Raises ValueError if the parentheses are unbalanced.
    """
    OPEN = 40
    CLOSE = 41

    depth = 0
    max_seen = 0
    for code in codes:
        if code == OPEN:
            depth += 1
            if depth > max_seen:
                max_seen = depth
        elif code == CLOSE:
            if depth == 0:
                raise ValueError("unbalanced parentheses: close without matching open")
            depth -= 1

    if depth != 0:
        raise ValueError("unbalanced parentheses: open never closed")

    return max_seen
