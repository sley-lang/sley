package solution

import "errors"

func SecondLargestDistinct(xs []int64) (int64, error) {
	if len(xs) == 0 {
		return 0, errors.New("fewer than two distinct values")
	}
	max := xs[0]
	for _, x := range xs {
		if x > max {
			max = x
		}
	}
	found := false
	var second int64
	for _, x := range xs {
		if x < max && (!found || x > second) {
			second = x
			found = true
		}
	}
	if !found {
		return 0, errors.New("fewer than two distinct values")
	}
	return second, nil
}
