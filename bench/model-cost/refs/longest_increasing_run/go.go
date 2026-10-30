package solution

func LongestIncreasingRun(xs []int64) int64 {
	if len(xs) == 0 {
		return 0
	}
	best, cur := int64(1), int64(1)
	for i := 1; i < len(xs); i++ {
		if xs[i] > xs[i-1] {
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
