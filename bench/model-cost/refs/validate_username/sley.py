def validate_username(s: str) -> int:
    LOWER = "abcdefghijklmnopqrstuvwxyz"
    ALLOWED = LOWER + "0123456789_"
    if not 3 <= len(s) <= 16:
        return 0
    if s[0] not in LOWER:
        return 0
    if any(c not in ALLOWED for c in s):
        return 0
    for i in range(len(s) - 1):
        if s[i] == "_" and s[i + 1] == "_":
            return 0
    if s[-1] == "_":
        return 0
    return 1
