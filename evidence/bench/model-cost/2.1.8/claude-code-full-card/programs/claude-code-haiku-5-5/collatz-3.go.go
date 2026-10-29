package solution

// Collatz returns the starting value n, 1 <= n < limit, whose Collatz
// sequence takes the most steps to reach 1. Ties go to the smallest n.
// It returns 0 when limit <= 1.
func Collatz(limit int64) int64 {
	if limit <= 1 {
		return 0
	}

	// cache[v] holds the step count for v when v < limit; -1 means unknown.
	cache := make([]int32, limit)
	for i := range cache {
		cache[i] = -1
	}
	cache[1] = 0

	// steps returns the number of steps for x to reach 1, memoizing
	// results for values below limit along the way.
	steps := func(x int64) int64 {
		var path []int64
		for x != 1 && (x >= limit || cache[x] < 0) {
			path = append(path, x)
			if x%2 == 0 {
				x /= 2
			} else {
				x = 3*x + 1
			}
		}

		var s int64
		if x != 1 {
			s = int64(cache[x])
		}

		for i := len(path) - 1; i >= 0; i-- {
			s++
			if path[i] < limit {
				cache[path[i]] = int32(s)
			}
		}
		return s
	}

	best := int64(1)
	bestSteps := steps(1)
	for n := int64(2); n < limit; n++ {
		s := steps(n)
		if s > bestSteps {
			bestSteps = s
			best = n
		}
	}
	return best
}
