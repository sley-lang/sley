package calc

func loopsum(n int64) int64 {
	var s int64
	for i := int64(0); i < n; i++ {
		s = (s + i*i) % 1000000007
	}
	return s
}
