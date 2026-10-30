package solution

import "errors"

func CountByRemainder(xs []int64, m int64) ([]int64, error) {
	if m < 1 || m > 200 {
		return nil, errors.New("m must be in [1, 200]")
	}
	out := make([]int64, m)
	for _, x := range xs {
		r := ((x % m) + m) % m
		out[r]++
	}
	return out, nil
}
