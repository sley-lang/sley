package solution

import "math"

// PairCount returns the number of index pairs i < j with xs[i] + xs[j] == k.
func PairCount(xs []int64, k int64) int64 {
	seen := make(map[int64]int64)
	var count int64
	for _, x := range xs {
		// need = k - x must be a valid int64 for a partner to exist.
		// Skip when the subtraction would overflow, since no partner can match.
		var need int64
		ok := true
		if x < 0 && k > math.MaxInt64+x {
			ok = false
		} else if x > 0 && k < math.MinInt64+x {
			ok = false
		} else {
			need = k - x
		}
		if ok {
			count += seen[need]
		}
		seen[x]++
	}
	return count
}
