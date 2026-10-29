package solution

// Isqrt returns the sum, over every integer i with 0 <= i < n, of the
// integer square root of i (the largest r with r*r <= i).
func Isqrt(n int64) int64 {
	var sum int64
	var r int64
	for i := int64(0); i < n; i++ {
		for (r+1)*(r+1) <= i {
			r++
		}
		sum += r
	}
	return sum
}
