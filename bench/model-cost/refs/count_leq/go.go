package solution

import "sort"

func CountLeq(xs []int64, x int64) int64 {
	return int64(sort.Search(len(xs), func(i int) bool { return xs[i] > x }))
}
