VALUES = [
    (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"),
    (100, "C"), (90, "XC"), (50, "L"), (40, "XL"),
    (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
]


def to_roman(n: int) -> str:
    if n < 1 or n > 3999:
        raise ValueError("out of range")
    parts = []
    for value, symbol in VALUES:
        while n >= value:
            parts.append(symbol)
            n -= value
    return "".join(parts)
