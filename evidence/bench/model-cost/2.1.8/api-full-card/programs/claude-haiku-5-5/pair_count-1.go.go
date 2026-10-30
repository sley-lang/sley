package solution

func PairCount(xs []int64, k int64) int64 {
	counts := make(map[int64]int64)
	var total int64
	for _, x := range xs {
		total += counts[k-x]
		counts[x]++
	}
	return total
}
