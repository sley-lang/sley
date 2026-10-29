package solution

func PairCount(xs []int64, k int64) int64 {
	var count int64
	for i := range xs {
		for j := i + 1; j < len(xs); j++ {
			if xs[i]+xs[j] == k {
				count++
			}
		}
	}
	return count
}
