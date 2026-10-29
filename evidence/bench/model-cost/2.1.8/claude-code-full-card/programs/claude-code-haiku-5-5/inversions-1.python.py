def inversions(xs: list[int]) -> int:
    """Return the number of index pairs i < j with xs[i] > xs[j].

    Uses a merge sort that counts cross-inversions during merging,
    giving O(n log n) time.
    """
    def sort_count(arr: list[int]) -> tuple[list[int], int]:
        n = len(arr)
        if n <= 1:
            return list(arr), 0
        mid = n // 2
        left, inv_left = sort_count(arr[:mid])
        right, inv_right = sort_count(arr[mid:])
        merged = []
        inv = inv_left + inv_right
        i = j = 0
        while i < len(left) and j < len(right):
            if left[i] <= right[j]:
                merged.append(left[i])
                i += 1
            else:
                # right[j] is strictly smaller than every remaining left element
                merged.append(right[j])
                inv += len(left) - i
                j += 1
        merged.extend(left[i:])
        merged.extend(right[j:])
        return merged, inv

    _, total = sort_count(list(xs))
    return total
