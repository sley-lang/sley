def format_thousands(n: int) -> str:
    num = n if n >= 0 else -n
    digits = str(num)
    groups: list[str] = []
    while len(digits) > 3:
        groups.append(digits[len(digits) - 3:len(digits)])
        digits = digits[0:len(digits) - 3]
    groups.append(digits)
    out: list[str] = []
    i = len(groups) - 1
    while i >= 0:
        out.append(groups[i])
        i -= 1
    result = ",".join(out)
    if n < 0:
        result = "-" + result
    return result
