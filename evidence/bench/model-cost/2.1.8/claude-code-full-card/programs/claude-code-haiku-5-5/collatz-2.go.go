package solution

// Collatz returns the starting value n, 1 <= n < limit, whose Collatz
// sequence takes the most steps to reach 1. Ties go to the smallest n.
// It returns 0 when limit <= 1.
func Collatz(limit int64) int64 {
	if limit <= 1 {
		return 0
	}

	lim := uint64(limit)
	// cache[v] holds the step count for v (when v < limit); 0 means unknown.
	// cache[1] stays 0, which is correct because 1 takes 0 steps.
	cache := make([]int32, limit)

	best := int64(1)
	var bestSteps int32 = 0

	for i := int64(2); i < limit; i++ {
		s := collatzSteps(uint64(i), cache, lim)
		if s > bestSteps {
			bestSteps = s
			best = i
		}
	}
	return best
}

// collatzSteps returns the number of steps for n to reach 1, memoizing
// results for values below lim in cache.
func collatzSteps(n uint64, cache []int32, lim uint64) int32 {
	var path []uint64

	for n != 1 {
		if n < lim && cache[n] != 0 {
			break
		}
		path = append(path, n)
		if n%2 == 0 {
			n /= 2
		} else {
			n = 3*n + 1
		}
	}

	var base int32
	if n != 1 {
		base = cache[n]
	}

	for i := len(path) - 1; i >= 0; i-- {
		base++
		if path[i] < lim {
			cache[path[i]] = base
		}
	}
	return base
}
