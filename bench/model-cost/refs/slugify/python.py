import re


def slugify(s: str) -> str:
    lowered = "".join(c.lower() if "A" <= c <= "Z" else c for c in s)
    return re.sub(r"[^a-z0-9]+", "-", lowered).strip("-")
