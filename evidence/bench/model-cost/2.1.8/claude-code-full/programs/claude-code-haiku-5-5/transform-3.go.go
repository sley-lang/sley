package solution

import "math"

// Transform maps each element x of xs to 2x if x is odd, or x if x is even,
// clamps the result into [lo, hi], and returns the sum of the clamped values.
// The doubling saturates instead of overflowing, so clamping stays correct for
// extreme inputs. The sum itself is not overflow-checked.
func Transform(xs []int64, lo int64, hi int64) int64 {
	var sum int64
	for _, x := range xs {
		var y int64
		if x%2 != 0 {
			switch {
			case x > math.MaxInt64/2:
				y = math.MaxInt64
			case x < math.MinInt64/2:
				y = math.MinInt64
			default:
				y = 2 * x
			}
		} else {
			y = x
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
