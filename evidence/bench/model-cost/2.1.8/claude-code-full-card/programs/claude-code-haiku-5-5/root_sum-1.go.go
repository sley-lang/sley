package solution

// RootSum returns the sum of the digital roots of 1..n inclusive.
// The digital root of k (k >= 1) is 1 + (k-1) % 9.
func RootSum(n int64) int64 {
	var total int64
	for k := int64(1); k <= n; k++ {
		total += 1 + (k-1)%9
	}
	return total
}
