package solution

// Isqrt returns the sum of floor(sqrt(i)) for every integer i with 0 <= i < n.
func Isqrt(n int64) int64 {
	var total int64
	var r int64 = 0
	for i := int64(0); i < n; i++ {
		// Advance r while (r+1)^2 <= i so that r == floor(sqrt(i)).
		for (r+1)*(r+1) <= i {
			r++
		}
		total += r
	}
	return total
}
