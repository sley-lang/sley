package calc

func inversions(xs []int64) int64 {
	var count int64
	for i, x := range xs {
		for j := i + 1; j < len(xs); j++ {
			if xs[j] < x {
				count++
			}
		}
	}
	return count
}
