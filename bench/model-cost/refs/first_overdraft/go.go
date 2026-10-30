package solution

func FirstOverdraft(start int64, xs []int64) int64 {
	balance := start
	for i, x := range xs {
		balance += x
		if balance < 0 {
			return int64(i)
		}
	}
	return -1
}
