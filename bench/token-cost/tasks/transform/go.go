package calc

func transform(xs []int64, lo, hi int64) int64 {
	var s int64
	for _, x := range xs {
		if x%2 != 0 {
			x *= 2
		}
		s += min(max(x, lo), hi)
	}
	return s
}
