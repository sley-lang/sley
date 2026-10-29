package solution

import "math"

// Transform doubles each odd element, leaves even elements unchanged,
// clamps each result into [lo, hi], and returns the sum of the clamped values.
// Doubling saturates at the int64 limits so that it never silently wraps.
func Transform(xs []int64, lo int64, hi int64) int64 {
	var sum int64
	for _, x := range xs {
		y := x
		if x%2 != 0 {
			switch {
			case x > math.MaxInt64/2:
				y = math.MaxInt64
			case x < math.MinInt64/2:
				y = math.MinInt64
			default:
				y = 2 * x
			}
		}
		if y < lo {
			y = lo
		} else if y > hi {
			y = hi
		}
		sum += y
	}
	return sum
}
