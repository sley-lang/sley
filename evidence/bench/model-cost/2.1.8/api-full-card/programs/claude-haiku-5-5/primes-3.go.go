package solution

func Primes(n int64) int64 {
	if n <= 2 {
		return 0
	}
	sieve := make([]bool, n)
	var count int64
	for i := int64(2); i < n; i++ {
		if !sieve[i] {
			count++
			for j := i * i; j < n; j += i {
				sieve[j] = true
			}
		}
	}
	return count
}
