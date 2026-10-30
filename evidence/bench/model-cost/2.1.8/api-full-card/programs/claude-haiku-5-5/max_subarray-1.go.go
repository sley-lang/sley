package solution

import "errors"

// MaxSubarray returns the largest sum of a non-empty contiguous subslice of xs.
// It returns an error if xs is empty.
func MaxSubarray(xs []int64) (int64, error) {
	if len(xs) == 0 {
		return 0, errors.New("empty input")
	}
	best := xs[0]
	cur := xs[0]
	for _, x := range xs[1:] {
		if cur < 0 {
			cur = x
		} else {
			cur += x
		}
		if cur > best {
			best = cur
		}
	}
	return best, nil
}
