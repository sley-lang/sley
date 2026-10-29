package solution

// Transform doubles each odd element of xs, leaves even elements unchanged,
// clamps each result into [lo, hi], and returns the sum of the clamped values.
func Transform(xs []int64, lo int64, hi int64) int64 {
	const maxInt64 = int64(^uint64(0) >> 1)
	const minInt64 = -maxInt64 - 1

	clamp := func(v int64) int64 {
		if v < lo {
			return lo
		}
		if v > hi {
			return hi
		}
		return v
	}

	var sum int64
	for _, x := range xs {
		var v int64
		if x%2 != 0 {
			// Avoid overflow when doubling: the doubled value would fall
			// outside [lo, hi] anyway, so saturate directly to the bound.
			switch {
			case x > maxInt64/2:
				v = hi
			case x < minInt64/2:
				v = lo
			default:
				v = clamp(2 * x)
			}
		} else {
			v = clamp(x)
		}
		sum += v
	}
	return sum
}
