package solution

func CountPairsWithSum(xs []int64, target int64) int64 {
	var count int64
	for i := 0; i < len(xs); i++ {
		for j := i + 1; j < len(xs); j++ {
			if xs[i]+xs[j] == target {
				count++
			}
		}
	}
	return count
}
