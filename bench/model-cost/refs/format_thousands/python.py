def format_thousands(n: int) -> str:
    digits = str(abs(n))
    groups = []
    while len(digits) > 3:
        groups.append(digits[-3:])
        digits = digits[:-3]
    groups.append(digits)
    result = ",".join(reversed(groups))
    return "-" + result if n < 0 else result
