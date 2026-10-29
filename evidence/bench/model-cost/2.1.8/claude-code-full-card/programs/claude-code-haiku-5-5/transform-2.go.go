package solution

import "math"

// Transform doubles odd elements, clamps each value into [lo, hi], and sums them.
func Transform(xs []int64, lo int64, hi int64) int64 {
	var sum int64
	for _, x := range xs {
		v := x
		if x&1 != 0 {
			switch {
			case x > math.MaxInt64/2:
				v = math.MaxInt64
			case x < math.MinInt64/2:
				v = math.MinInt64
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
