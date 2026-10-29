package solution

// PairCount returns the number of index pairs i < j with xs[i] + xs[j] == k.
func PairCount(xs []int64, k int64) int64 {
	counts := make(map[int64]int64, len(xs))
	var total int64
	for _, x := range xs {
		// Pairs formed with an earlier element y where y + x == k.
		total += counts[k-x]
		counts[x]++
	}
	return total
}
