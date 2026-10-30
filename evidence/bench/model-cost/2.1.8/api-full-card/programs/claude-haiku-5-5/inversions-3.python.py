def inversions(xs: list[int]) -> int:
    def sort_count(arr):
        n = len(arr)
        if n <= 1:
            return arr, 0
        mid = n // 2
        left, inv_left = sort_count(arr[:mid])
        right, inv_right = sort_count(arr[mid:])
        merged = []
        i = j = 0
        inv = inv_left + inv_right
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

    _, total = sort_count(list(xs))
    return total
