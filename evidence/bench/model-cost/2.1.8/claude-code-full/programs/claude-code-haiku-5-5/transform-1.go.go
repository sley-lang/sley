package solution

func Transform(xs []int64, lo int64, hi int64) int64 {
	var sum int64
	for _, x := range xs {
		v := x
		if x%2 != 0 {
			v = 2 * x
		}
		if v < lo {
			v = lo
		} else if v > hi {
			v = hi
		}
		sum += v
	}
	return sum
}
