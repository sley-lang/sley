package solution

import "math"

// Transform doubles each odd element, leaves even elements unchanged,
// clamps each result into [lo, hi], and returns the sum of the clamped values.
func Transform(xs []int64, lo int64, hi int64) int64 {
	var sum int64
	for _, x := range xs {
		var v int64
		if x%2 != 0 {
			// Saturate instead of overflowing; clamping then yields the correct bound.
			switch {
			case x > math.MaxInt64/2:
				v = math.MaxInt64
			case x < math.MinInt64/2:
				v = math.MinInt64
			default:
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
