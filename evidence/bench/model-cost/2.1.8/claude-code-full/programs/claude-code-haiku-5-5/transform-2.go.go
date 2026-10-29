package solution

// Transform doubles each odd element of xs (leaving even elements unchanged),
// clamps each result into [lo, hi], and returns the sum of the clamped values.
// Doubling saturates at the int64 bounds, so no intermediate value wraps.
func Transform(xs []int64, lo int64, hi int64) int64 {
	const maxI = int64(^uint64(0) >> 1)
	const minI = -maxI - 1

	var sum int64
	for _, x := range xs {
		v := x
		if x&1 != 0 { // odd (also correct for negative values in two's complement)
			switch {
			case x > maxI/2:
				v = maxI
			case x < minI/2:
				v = minI
			default:
				v = 2 * x
			}
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
