package solution

func Inversions(xs []int64) int64 {
	var count int64
	for i := range xs {
		for j := i + 1; j < len(xs); j++ {
			if xs[j] < xs[i] {
				count++
			}
		}
	}
	return count
}
