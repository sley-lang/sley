def parse_int(chars: list[int]) -> int:
    if not chars:
        raise ValueError("empty input")

    negative = False
    digits = chars
    if chars[0] == 45:
        negative = True
        digits = chars[1:]

    if not digits:
        raise ValueError("no digits after minus sign")

    value = 0
    for code in digits:
        if not (48 <= code <= 57):
            raise ValueError(f"invalid character code: {code}")
        value = value * 10 + (code - 48)

    return -value if negative else value
