package solution

// Isqrt returns the sum of floor(sqrt(i)) for every integer i with 0 <= i < n.
func Isqrt(n int64) int64 {
	var total int64
	var r int64 // current integer square root; r*r <= i < (r+1)*(r+1)
	for i := int64(0); i < n; i++ {
		for (r+1)*(r+1) <= i {
			r++
		}
		total += r
	}
	return total
}
