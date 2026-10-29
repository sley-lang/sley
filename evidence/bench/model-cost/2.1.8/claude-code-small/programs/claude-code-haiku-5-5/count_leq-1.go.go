package solution

// CountLeq returns the number of elements in the sorted slice xs that are
// less than or equal to x. It uses binary search to find the first index
// whose element is strictly greater than x.
func CountLeq(xs []int64, x int64) int64 {
	lo, hi := 0, len(xs)
	for lo < hi {
		mid := lo + (hi-lo)/2
		if xs[mid] <= x {
			lo = mid + 1
		} else {
			hi = mid
		}
	}
	return int64(lo)
}
