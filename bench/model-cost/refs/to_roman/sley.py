def to_roman(n: int) -> str:
    if n < 1 or n > 3999:
        raise ValueError("out of range")
    vals: list[int] = [1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1]
    syms: list[str] = ["M", "CM", "D", "CD", "C", "XC", "L", "XL", "X", "IX", "V", "IV", "I"]
    parts: list[str] = []
    i = 0
    while i < len(vals):
        while n >= vals[i]:
            parts.append(syms[i])
            n -= vals[i]
        i += 1
    return "".join(parts)
