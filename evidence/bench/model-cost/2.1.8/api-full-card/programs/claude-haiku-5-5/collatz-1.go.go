package solution

// Collatz returns the starting value n, 1 <= n < limit, whose Collatz
// sequence takes the most steps to reach 1. Ties go to the smallest n.
// Returns 0 when limit <= 1.
func Collatz(limit int64) int64 {
	if limit <= 1 {
		return 0
	}
	// steps[i] holds the number of steps for i to reach 1, for i < limit.
	steps := make([]int32, limit)
	steps[1] = 0

	best := int64(1)
	bestSteps := int32(0)

	for n := int64(2); n < limit; n++ {
		x := n
		var count int32
		// Every value below n already has a memoized result, so we can
		// stop as soon as the trajectory drops below n.
		for x >= n {
			if x%2 == 0 {
				x /= 2
			} else {
				x = 3*x + 1
			}
			count++
		}
		total := count + steps[x]
		steps[n] = total
		if total > bestSteps {
			bestSteps = total
			best = n
		}
	}
	return best
}
