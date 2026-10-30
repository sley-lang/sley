package solution

import "math"

// Transform doubles each odd element, leaves even elements unchanged,
// clamps every result into [lo, hi], and returns the sum.
func Transform(xs []int64, lo int64, hi int64) int64 {
	var sum int64
	for _, x := range xs {
		var v int64
		if x&1 != 0 {
			// Odd: value is 2x. Avoid overflow by deciding the clamp
			// directly when 2x would not fit in int64.
			if x > math.MaxInt64/2 {
				v = hi
			} else if x < math.MinInt64/2 {
				v = lo
			} else {
				v = 2 * x
			}
		} else {
			v = x
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
