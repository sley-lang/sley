package solution

func ClampSum(xs []int64, lo, hi int64) (int64, error) {
	var total int64
	for _, x := range xs {
		total += min(max(x, lo), hi)
	}
	return total, nil
}
