package solution

func Primes(n int64) int64 {
	var count int64
	for k := int64(2); k < n; k++ {
		isP := true
		for d := int64(2); d*d <= k; d++ {
			if k%d == 0 {
				isP = false
				break
			}
		}
		if isP {
			count++
		}
	}
	return count
}
