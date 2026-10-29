package solution

func Transform(xs []int64, lo, hi int64) int64 {
	var total int64
	for _, x := range xs {
		if x%2 != 0 {
			x *= 2
		}
		total += min(max(x, lo), hi)
	}
	return total
}
