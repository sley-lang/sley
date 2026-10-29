package solution

// LongestRun returns the length of the longest run of equal adjacent
// elements in xs. It returns 0 for an empty slice.
func LongestRun(xs []int64) int64 {
	if len(xs) == 0 {
		return 0
	}

	var best, cur int64
	for i, x := range xs {
		if i > 0 && x == xs[i-1] {
			cur++
		} else {
			cur = 1
		}
		if cur > best {
			best = cur
		}
	}
	return best
}
