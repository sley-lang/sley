def parse_int(chars: list[int]) -> int:
    n = len(chars)
    if n == 0:
        raise ValueError("empty input")
    neg = False
    start = 0
    if chars[0] == 45:
        neg = True
        start = 1
    if start == n:
        raise ValueError("no digits")
    v = 0
    for k in range(start, n):
        c = chars[k]
        if not (48 <= c <= 57):
            raise ValueError("invalid character")
        v = v * 10 + (c - 48)
    if neg:
        return -v
    return v
