package solution

// Digitsum returns the total of the decimal digits of every integer i
// with 0 <= i < n.
func Digitsum(n int64) int64 {
	var total int64
	for i := int64(0); i < n; i++ {
		x := i
		for x > 0 {
			total += x % 10
			x /= 10
		}
	}
	return total
}
