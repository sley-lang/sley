package calc

func triples(n int64) int64 {
	var count int64
	for c := int64(1); c <= n; c++ {
		for b := int64(1); b < c; b++ {
			for a := int64(1); a < b; a++ {
				if a*a+b*b == c*c {
					count++
				}
			}
		}
	}
	return count
}
