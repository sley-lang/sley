package calc

func gcd(a, b int64) int64 {
	for b != 0 {
		a, b = b, a%b
	}
	return a
}

func gcdsum(n int64) int64 {
	var s int64
	for i := int64(1); i < n; i++ {
		for j := int64(1); j < n; j++ {
			s += gcd(i, j)
		}
	}
	return s
}
