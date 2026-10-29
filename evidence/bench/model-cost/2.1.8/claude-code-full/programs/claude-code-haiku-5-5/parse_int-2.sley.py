def parse_int(chars: list[int]) -> int:
    if len(chars) == 0:
        raise ValueError("empty input")
    start = 0
    negative = False
    if chars[0] == 45:
        negative = True
        start = 1
    if start == len(chars):
        raise ValueError("no digits")
    result = 0
    for i in range(start, len(chars)):
        c = chars[i]
        if c < 48 or c > 57:
            raise ValueError("invalid character")
        result = result * 10 + (c - 48)
    if negative:
        return -result
    return result
