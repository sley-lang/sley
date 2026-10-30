package solution

func Collatz(limit int64) int64 {
	if limit <= 1 {
		return 0
	}
	n := int(limit)
	// cache[v] = number of steps for v to reach 1, valid for v < limit.
	cache := make([]int32, n)
	cache[1] = 0

	best := int64(1)
	bestSteps := int32(0)

	for i := 2; i < n; i++ {
		x := int64(i)
		var count int32
		// Walk until we drop below i, whose step count is already cached.
		for x >= int64(i) {
			if x%2 == 0 {
				x /= 2
			} else {
				x = 3*x + 1
			}
			count++
		}
		total := count + cache[x]
		cache[i] = total
		if total > bestSteps {
			bestSteps = total
			best = int64(i)
		}
	}
	return best
}
