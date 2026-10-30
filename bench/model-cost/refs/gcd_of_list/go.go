package solution

func GcdOfList(xs []int64) int64 {
	var g int64
	for _, x := range xs {
		if x < 0 {
			x = -x
		}
		a, b := g, x
		for b != 0 {
			a, b = b, a%b
		}
		g = a
	}
	return g
}
