package solution

func CountMultiples(xs []int64, k int64) (int64, error) {
	var count int64
	for _, x := range xs {
		if x%2 == 0 {
			count++
		}
	}
	return count, nil
}
