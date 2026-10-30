def parse_signed_int(s: str) -> int:
    body = s
    sign = 1
    if body[:1] in ("+", "-"):
        if body[0] == "-":
            sign = -1
        body = body[1:]
    if not body or any(c not in "0123456789" for c in body):
        raise ValueError("invalid integer")
    value = sign * int(body)
    if value < -1000000 or value > 1000000:
        raise ValueError("out of range")
    return value
