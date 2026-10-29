package solution

import "errors"

func ModeValue(xs []int64) (int64, error) {
	if len(xs) == 0 {
		return 0, errors.New("empty")
	}
	counts := map[int64]int{}
	for _, x := range xs {
		counts[x]++
	}
	best, bestCount := xs[0], 0
	for v, c := range counts {
		if c > bestCount || (c == bestCount && v < best) {
			best, bestCount = v, c
		}
	}
	return best, nil
}
