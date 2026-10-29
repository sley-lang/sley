package solution

import "sort"

// CountLeq returns how many elements of the sorted slice xs are less than or equal to x.
func CountLeq(xs []int64, x int64) int64 {
	idx := sort.Search(len(xs), func(i int) bool {
		return xs[i] > x
	})
	return int64(idx)
}
