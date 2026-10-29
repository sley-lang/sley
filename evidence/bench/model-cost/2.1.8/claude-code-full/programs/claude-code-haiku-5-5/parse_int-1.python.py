def parse_int(chars: list[int]) -> int:
    """Parse a list of character codes as a decimal integer.

    An optional leading minus sign (code 45) is allowed, followed by one or
    more digits (codes 48 to 57). Raises ValueError on any other input.
    """
    if not chars:
        raise ValueError("empty input")

    negative = False
    start = 0
    if chars[0] == 45:
        negative = True
        start = 1

    if start == len(chars):
        raise ValueError("no digits after minus sign")

    value = 0
    for code in chars[start:]:
        if not (48 <= code <= 57):
            raise ValueError(f"invalid character code: {code}")
        value = value * 10 + (code - 48)

    return -value if negative else value
