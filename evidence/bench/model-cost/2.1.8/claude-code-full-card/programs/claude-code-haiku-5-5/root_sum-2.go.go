package solution

// RootSum returns the sum of the digital roots of the integers 1 through n inclusive.
func RootSum(n int64) int64 {
	var total int64
	for k := int64(1); k <= n; k++ {
		x := k
		for x >= 10 {
			var s int64
			for y := x; y > 0; y /= 10 {
				s += y % 10
			}
			x = s
		}
		total += x
	}
	return total
}
