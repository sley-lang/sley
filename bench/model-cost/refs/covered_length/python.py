def covered_length(xs: list[int]) -> int:
    if len(xs) % 2 != 0:
        raise ValueError("odd number of elements")
    intervals = []
    for i in range(0, len(xs), 2):
        start, end = xs[i], xs[i + 1]
        if start > end:
            raise ValueError("start greater than end")
        if start < end:
            intervals.append((start, end))
    intervals.sort()
    total = 0
    cur_start = cur_end = None
    for start, end in intervals:
        if cur_end is None or start > cur_end:
            if cur_end is not None:
                total += cur_end - cur_start
            cur_start, cur_end = start, end
        else:
            cur_end = max(cur_end, end)
    if cur_end is not None:
        total += cur_end - cur_start
    return total
