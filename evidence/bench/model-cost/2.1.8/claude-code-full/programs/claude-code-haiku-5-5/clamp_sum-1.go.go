package solution

import "errors"

func ClampSum(xs []int64, lo, hi int64) (int64, error) {
	if lo > hi {
		return 0, errors.New("ClampSum: lo must not be greater than hi")
	}
	var total int64
	for _, x := range xs {
		if x < 0 {
			continue
		}
		total += min(max(x, lo), hi)
	}
	return total, nil
}
