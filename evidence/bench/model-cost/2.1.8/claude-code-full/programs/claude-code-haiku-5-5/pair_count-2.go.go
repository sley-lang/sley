package solution

// PairCount returns the number of index pairs i < j with xs[i] + xs[j] == k.
// Sums use Go's wrapping int64 arithmetic, matching the expression xs[i] + xs[j].
func PairCount(xs []int64, k int64) int64 {
	seen := make(map[int64]int64, len(xs))
	var count int64
	for _, x := range xs {
		// The complement w satisfies x + w == k under wrapping addition.
		w := k - x
		count += seen[w]
		seen[x]++
	}
	return count
}
