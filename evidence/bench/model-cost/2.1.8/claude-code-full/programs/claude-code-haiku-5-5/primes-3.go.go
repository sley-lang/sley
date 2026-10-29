package solution

// Primes returns the number of prime numbers strictly less than n.
// Valid for 0 <= n <= 2000 (works for any non-negative n using a sieve).
func Primes(n int64) int64 {
	if n <= 2 {
		return 0
	}
	limit := int(n)
	composite := make([]bool, limit)
	var count int64
	for i := 2; i < limit; i++ {
		if !composite[i] {
			count++
			for j := i * i; j < limit; j += i {
				composite[j] = true
			}
		}
	}
	return count
}
