def parse_signed_int(s: str) -> int:
    sign = 1
    start = 0
    if len(s) > 0 and (s[0] == "+" or s[0] == "-"):
        if s[0] == "-":
            sign = -1
        start = 1
    if len(s) - start == 0:
        raise ValueError("invalid integer")
    value = 0
    for i in range(start, len(s)):
        c = s[i]
        if not ("0" <= c <= "9"):
            raise ValueError("invalid integer")
        value = value * 10 + (ord(c) - 48)
    value = sign * value
    if value < -1000000 or value > 1000000:
        raise ValueError("out of range")
    return value
