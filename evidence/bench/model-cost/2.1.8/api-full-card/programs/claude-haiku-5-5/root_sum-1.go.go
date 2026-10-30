package solution

func RootSum(n int64) int64 {
	var total int64
	for k := int64(1); k <= n; k++ {
		total += 1 + (k-1)%9
	}
	return total
}
