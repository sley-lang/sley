package calc

func isqrt(n int64) int64 {
	var total int64
	for i := int64(0); i < n; i++ {
		lo, hi := int64(0), i+1
		for hi-lo > 1 {
			mid := (lo + hi) / 2
			if mid*mid <= i {
				lo = mid
			} else {
				hi = mid
			}
		}
		total += lo
	}
	return total
}
