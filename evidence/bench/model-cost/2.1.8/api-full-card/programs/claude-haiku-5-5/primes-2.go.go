package solution

func Primes(n int64) int64 {
	if n <= 2 {
		return 0
	}
	limit := int(n)
	sieve := make([]bool, limit)
	var count int64
	for i := 2; i < limit; i++ {
		if !sieve[i] {
			count++
			for j := i * i; j < limit; j += i {
				sieve[j] = true
			}
		}
	}
	return count
}
