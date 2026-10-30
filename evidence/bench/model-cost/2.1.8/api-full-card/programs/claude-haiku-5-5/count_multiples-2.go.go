package solution

import "errors"

func CountMultiples(xs []int64, k int64) (int64, error) {
	if k <= 0 {
		return 0, errors.New("k must be positive")
	}
	var count int64
	for _, x := range xs {
		if x%k == 0 {
			count++
		}
	}
	return count, nil
}
