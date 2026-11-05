def slugify(s: str) -> str:
    lowered = ""
    for c in s:
        if "A" <= c <= "Z":
            lowered += chr(ord(c) + 32)
        else:
            lowered += c
    out = ""
    prev_sep = False
    for c in lowered:
        if ("a" <= c <= "z") or ("0" <= c <= "9"):
            out += c
            prev_sep = False
        elif not prev_sep:
            out += "-"
            prev_sep = True
    if len(out) > 0 and out[0] == "-":
        out = out[1:len(out)]
    while len(out) > 0 and out[len(out) - 1] == "-":
        out = out[0:len(out) - 1]
    return out
