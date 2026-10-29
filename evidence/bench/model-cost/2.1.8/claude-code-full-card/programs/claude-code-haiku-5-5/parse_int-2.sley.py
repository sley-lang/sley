def parse_int(chars: list[int]) -> int:
    n = len(chars)
    if n == 0:
        raise ValueError("empty input")
    start = 0
    negative = False
    if chars[0] == 45:
        negative = True
        start = 1
    if start == n:
        raise ValueError("no digits")
    value = 0
    i = start
    while i < n:
        c = chars[i]
        if c < 48 or c > 57:
            raise ValueError("invalid character")
        value = value * 10 + (c - 48)
        i += 1
    if negative:
        return -value
    return value
