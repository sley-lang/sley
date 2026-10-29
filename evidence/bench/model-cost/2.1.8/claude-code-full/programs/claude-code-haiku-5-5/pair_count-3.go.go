package solution

// PairCount returns the number of index pairs i < j with xs[i] + xs[j] == k.
func PairCount(xs []int64, k int64) int64 {
	seen := make(map[int64]int64, len(xs))
	var total int64
	for _, x := range xs {
		need := k - x
		// If k - x falls outside the int64 range, the subtraction wrapped
		// around, so no value in xs can complete the pair.
		overflow := (x < 0 && k > 0 && need < 0) || (x > 0 && k < 0 && need > 0)
		if !overflow {
			total += seen[need]
		}
		seen[x]++
	}
	return total
}
