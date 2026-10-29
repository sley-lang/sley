package solution

// CountLeq returns the number of elements in xs that are less than or equal to x.
// xs must be sorted in ascending order. It uses binary search to find the first
// index whose element is greater than x; that index equals the count.
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
