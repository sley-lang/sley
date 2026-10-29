package solution

// Isqrt returns the sum of floor(sqrt(i)) for all integers i with 0 <= i < n.
func Isqrt(n int64) int64 {
	var total int64
	for i := int64(0); i < n; i++ {
		var r int64
		for (r+1)*(r+1) <= i {
			r++
		}
		total += r
	}
	return total
}
