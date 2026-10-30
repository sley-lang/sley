def parse_int(chars: list[int]) -> int:
    n = len(chars)
    if n == 0:
        raise ValueError("empty input")
    start = 0
    neg = False
    if chars[0] == 45:
        neg = True
        start = 1
    if start == n:
        raise ValueError("no digits")
    result = 0
    i = start
    while i < n:
        c = chars[i]
        if c < 48 or c > 57:
            raise ValueError("invalid character")
        result = result * 10 + (c - 48)
        i += 1
    if neg:
        result = -result
    return result
