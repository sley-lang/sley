def mask_digits(s: str) -> str:
    DIGITS = "0123456789"
    to_mask = sum(1 for c in s if c in DIGITS) - 4
    out: list[str] = []
    for c in s:
        if c in DIGITS and to_mask > 0:
            out.append("*")
            to_mask -= 1
        else:
            out.append(c)
    return "".join(out)
