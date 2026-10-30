package solution

func clamp(v, lo, hi int64) int64 {
	if v < lo {
		return lo
	}
	if v > hi {
		return hi
	}
	return v
}

func Transform(xs []int64, lo int64, hi int64) int64 {
	var sum int64
	for _, x := range xs {
		var r int64
		if x%2 != 0 {
			// Odd: result is 2x, clamped. Avoid overflow by deciding
			// the clamp direction before multiplying.
			if x > 0 && x > hi/2 {
				r = hi
			} else if x < 0 && x < lo/2 {
				r = lo
			} else {
				r = clamp(2*x, lo, hi)
			}
		} else {
			r = clamp(x, lo, hi)
		}
		sum += r
	}
	return sum
}
