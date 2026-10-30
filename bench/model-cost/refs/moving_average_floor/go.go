package solution

import "errors"

func MovingAverageFloor(xs []int64, w int64) ([]int64, error) {
	if w < 1 {
		return nil, errors.New("w must be at least 1")
	}
	out := []int64{}
	if w > int64(len(xs)) {
		return out, nil
	}
	n := int(w)
	for i := 0; i+n <= len(xs); i++ {
		var sum int64
		for _, x := range xs[i : i+n] {
			sum += x
		}
		q := sum / w
		if sum%w != 0 && sum < 0 {
			q--
		}
		out = append(out, q)
	}
	return out, nil
}
