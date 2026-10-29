package solution

// PairCount returns the number of index pairs i < j with xs[i] + xs[j] == k.
func PairCount(xs []int64, k int64) int64 {
	seen := make(map[int64]int64)
	var count int64
	for _, x := range xs {
		count += seen[k-x]
		seen[x]++
	}
	return count
}
