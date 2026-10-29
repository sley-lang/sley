package solution

func Collatz(limit int64) int64 {
	var best, bestN int64
	for n := int64(1); n < limit; n++ {
		x, steps := n, int64(0)
		for x != 1 {
			if x%2 == 0 {
				x /= 2
			} else {
				x = 3*x + 1
			}
			steps++
		}
		if steps > best {
			best, bestN = steps, n
		}
	}
	return bestN
}
