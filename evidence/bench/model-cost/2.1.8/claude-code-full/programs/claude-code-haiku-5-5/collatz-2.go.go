package solution

// Collatz returns the starting value n, with 1 <= n < limit, whose Collatz
// sequence takes the most steps to reach 1. On a tie, the smallest such n is
// returned. It returns 0 when limit <= 1.
func Collatz(limit int64) int64 {
	if limit <= 1 {
		return 0
	}

	lim := uint64(limit)

	// steps[i] holds the step count for i once known, or -1 if not yet computed.
	// Only values below limit are cached; larger intermediate values are walked through.
	steps := make([]int32, lim)
	for i := range steps {
		steps[i] = -1
	}
	steps[1] = 0

	// path records every value visited (including those >= limit) so that
	// step counts are accurate for the cached values along the walk.
	path := make([]uint64, 0, 512)
	for n := uint64(2); n < lim; n++ {
		if steps[n] >= 0 {
			continue
		}
		path = path[:0]
		x := n
		for x >= lim || steps[x] < 0 {
			path = append(path, x)
			if x%2 == 0 {
				x /= 2
			} else {
				x = 3*x + 1
			}
		}
		base := steps[x]
		for i := len(path) - 1; i >= 0; i-- {
			base++
			if path[i] < lim {
				steps[path[i]] = base
			}
		}
	}

	best := uint64(1)
	bestSteps := steps[1]
	for n := uint64(2); n < lim; n++ {
		if steps[n] > bestSteps {
			bestSteps = steps[n]
			best = n
		}
	}
	return int64(best)
}
