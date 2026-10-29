def inversions(xs: list[int]) -> int:
    """Return the number of index pairs i < j with xs[i] > xs[j]."""

    def _count(arr: list[int]) -> tuple[list[int], int]:
        n = len(arr)
        if n <= 1:
            return arr[:], 0
        mid = n // 2
        left, inv_left = _count(arr[:mid])
        right, inv_right = _count(arr[mid:])
        merged = []
        inv = inv_left + inv_right
        i = j = 0
        while i < len(left) and j < len(right):
            if left[i] <= right[j]:
                merged.append(left[i])
                i += 1
            else:
                # right[j] is smaller than all remaining left elements
                merged.append(right[j])
                inv += len(left) - i
                j += 1
        merged.extend(left[i:])
        merged.extend(right[j:])
        return merged, inv

    return _count(list(xs))[1]
