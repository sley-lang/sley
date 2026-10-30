package solution

import "errors"

// ModeValue returns the value that occurs most often in xs.
// On a tie, the smallest such value is returned.
// It returns an error if xs is empty.
func ModeValue(xs []int64) (int64, error) {
	if len(xs) == 0 {
		return 0, errors.New("empty input")
	}
	counts := make(map[int64]int, len(xs))
	for _, x := range xs {
		counts[x]++
	}
	var best int64
	bestCount := 0
	first := true
	for v, c := range counts {
		if first || c > bestCount || (c == bestCount && v < best) {
			best = v
			bestCount = c
			first = false
		}
	}
	return best, nil
}
