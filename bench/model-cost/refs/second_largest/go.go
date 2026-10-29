package solution

import "errors"

func SecondLargest(xs []int64) (int64, error) {
	if len(xs) == 0 {
		return 0, errors.New("too few values")
	}
	top := xs[0]
	for _, x := range xs {
		top = max(top, x)
	}
	found, second := false, int64(0)
	for _, x := range xs {
		if x != top && (!found || x > second) {
			found, second = true, x
		}
	}
	if !found {
		return 0, errors.New("too few values")
	}
	return second, nil
}
