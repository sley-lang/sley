def count_pairs_with_sum(xs: list[int], target: int) -> int:
    count = 0
    for i in range(len(xs)):
        for j in range(i + 1, len(xs)):
            if xs[i] + xs[j] == target:
                count += 1
    return count
