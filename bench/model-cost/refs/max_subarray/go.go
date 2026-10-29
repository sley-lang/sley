package solution

import "errors"

func MaxSubarray(xs []int64) (int64, error) {
	if len(xs) == 0 {
		return 0, errors.New("empty")
	}
	best, cur := xs[0], xs[0]
	for _, x := range xs[1:] {
		cur = max(x, cur+x)
		best = max(best, cur)
	}
	return best, nil
}
