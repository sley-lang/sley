package solution

func Digitsum(n int64) int64 {
	var total int64
	for i := int64(0); i < n; i++ {
		for x := i; x > 0; x /= 10 {
			total += x % 10
		}
	}
	return total
}
