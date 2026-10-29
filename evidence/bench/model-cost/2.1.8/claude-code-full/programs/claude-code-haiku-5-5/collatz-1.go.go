package solution

// Collatz returns the starting value n, with 1 <= n < limit, whose Collatz
// sequence takes the most steps to reach 1. Ties go to the smallest n.
// It returns 0 when limit <= 1.
func Collatz(limit int64) int64 {
	if limit <= 1 {
		return 0
	}

	n := int(limit)
	// steps[i] holds the number of steps for starting value i (steps[1] = 0).
	steps := make([]int32, n)

	var bestN int64 = 1
	var bestSteps int32 = 0

	for i := 2; i < n; i++ {
		// Walk the trajectory until it drops below i, whose step count is already known.
		x := uint64(i)
		var count int32
		for x >= uint64(i) {
			if x%2 == 0 {
				x /= 2
			} else {
				x = 3*x + 1
			}
			count++
		}
		total := count + steps[x]
		steps[i] = total

		// Strict comparison keeps the smallest n on ties.
		if total > bestSteps {
			bestSteps = total
			bestN = int64(i)
		}
	}

	return bestN
}
