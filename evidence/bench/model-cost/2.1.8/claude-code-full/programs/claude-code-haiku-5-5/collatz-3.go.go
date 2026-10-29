package solution

// Collatz returns the starting value n, 1 <= n < limit, whose Collatz
// sequence takes the most steps to reach 1. Ties go to the smallest n.
// It returns 0 when limit <= 1.
func Collatz(limit int64) int64 {
	if limit <= 1 {
		return 0
	}

	// cache[x] holds (steps to reach 1) + 1 for x < limit, or 0 if unknown.
	cache := make([]int32, limit)
	cache[1] = 1

	best := int64(1)
	var bestSteps int32 = 0

	for n := int64(2); n < limit; n++ {
		s := collatzSteps(n, limit, cache)
		if s > bestSteps {
			bestSteps = s
			best = n
		}
	}
	return best
}

// collatzSteps returns the number of steps needed to reach 1 from n,
// memoizing results for values below limit in cache.
func collatzSteps(n int64, limit int64, cache []int32) int32 {
	path := make([]int64, 0, 64)
	x := n
	for x >= limit || cache[x] == 0 {
		path = append(path, x)
		if x%2 == 0 {
			x /= 2
		} else {
			x = 3*x + 1
		}
	}

	// Value (steps + 1) for x, which is cached because x < limit here.
	s := cache[x]
	for i := len(path) - 1; i >= 0; i-- {
		s++
		if path[i] < limit {
			cache[path[i]] = s
		}
	}
	return s - 1
}
