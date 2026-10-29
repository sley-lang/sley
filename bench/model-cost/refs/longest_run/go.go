package solution

func LongestRun(xs []int64) int64 {
	var best, run int64
	for i := range xs {
		if i > 0 && xs[i] == xs[i-1] {
			run++
		} else {
			run = 1
		}
		best = max(best, run)
	}
	return best
}
