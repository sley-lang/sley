package solution

func RootSum(n int64) int64 {
	var total int64
	for i := int64(1); i <= n; i++ {
		if i%9 == 0 {
			total += 9
		} else {
			total += i % 9
		}
	}
	return total
}
