package solution

// Collatz returns the starting value n, 1 <= n < limit, whose Collatz
// sequence takes the most steps to reach 1. Ties go to the smallest n.
// It returns 0 when limit <= 1.
func Collatz(limit int64) int64 {
	if limit <= 1 {
		return 0
	}

	// cache[v] holds the step count for v when v < limit and it is known.
	// 0 means unknown, which is safe because every v >= 2 needs at least one step.
	cache := make([]int32, limit)
	path := make([]uint64, 0, 512)
	lim := uint64(limit)

	steps := func(start uint64) int32 {
		path = path[:0]
		cur := start
		for cur != 1 && (cur >= lim || cache[cur] == 0) {
			path = append(path, cur)
			if cur%2 == 0 {
				cur /= 2
			} else {
				cur = 3*cur + 1
			}
		}
		var base int32
		if cur != 1 {
			base = cache[cur]
		}
		for i := len(path) - 1; i >= 0; i-- {
			base++
			v := path[i]
			if v < lim {
				cache[v] = base
			}
		}
		return base
	}

	bestN := uint64(1)
	bestSteps := int32(0)
	for n := uint64(2); n < lim; n++ {
		s := steps(n)
		if s > bestSteps {
			bestSteps = s
			bestN = n
		}
	}

	return int64(bestN)
}
