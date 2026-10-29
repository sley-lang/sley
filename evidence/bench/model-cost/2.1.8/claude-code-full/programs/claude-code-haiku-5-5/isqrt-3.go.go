package solution

func Isqrt(n int64) int64 {
	var sum int64
	var r int64
	for i := int64(0); i < n; i++ {
		// Advance r so that r*r <= i < (r+1)*(r+1).
		for (r+1)*(r+1) <= i {
			r++
		}
		sum += r
	}
	return sum
}
