package solution

func Collatz(limit int64) int64 {
	if limit <= 1 {
		return 0
	}
	n := int(limit)
	steps := make([]int32, n)
	// steps[1] = 0 (already zero-valued)
	best := int64(1)
	var bestSteps int32 = 0
	for i := 2; i < n; i++ {
		x := int64(i)
		var c int32 = 0
		for x >= int64(i) {
			if x%2 == 0 {
				x /= 2
			} else {
				x = 3*x + 1
			}
			c++
		}
		total := c + steps[x]
		steps[i] = total
		if total > bestSteps {
			bestSteps = total
			best = int64(i)
		}
	}
	return best
}
