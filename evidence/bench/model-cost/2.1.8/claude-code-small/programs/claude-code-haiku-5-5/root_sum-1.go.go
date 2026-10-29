package solution

// RootSum returns the sum of the digital roots of the integers 1 through n inclusive.
func RootSum(n int64) int64 {
	var total int64
	for i := int64(1); i <= n; i++ {
		total += 1 + (i-1)%9
	}
	return total
}
