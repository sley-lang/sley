def inversions(xs: list[int]) -> int:
    def sort_count(a):
        n = len(a)
        if n <= 1:
            return a, 0
        mid = n // 2
        left, inv_l = sort_count(a[:mid])
        right, inv_r = sort_count(a[mid:])
        merged = []
        i = j = 0
        inv = inv_l + inv_r
        while i < len(left) and j < len(right):
            if left[i] <= right[j]:
                merged.append(left[i])
                i += 1
            else:
                merged.append(right[j])
                inv += len(left) - i
                j += 1
        merged.extend(left[i:])
        merged.extend(right[j:])
        return merged, inv

    _, count = sort_count(list(xs))
    return count
