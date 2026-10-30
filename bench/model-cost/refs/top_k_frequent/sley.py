def top_k_frequent(xs: list[int], k: int) -> list[int]:
    # No dicts: count each distinct value in parallel lists, then pick the
    # k best by (count descending, value ascending).
    vals = []
    counts = []
    for x in xs:
        found = False
        for i in range(len(vals)):
            if vals[i] == x:
                counts[i] += 1
                found = True
        if not found:
            vals.append(x)
            counts.append(1)
    used = [False] * len(vals)
    out = []
    while len(out) < k and len(out) < len(vals):
        best = -1
        for i in range(len(vals)):
            if not used[i]:
                if best < 0 or counts[i] > counts[best] or (counts[i] == counts[best] and vals[i] < vals[best]):
                    best = i
        used[best] = True
        out.append(vals[best])
    return out
