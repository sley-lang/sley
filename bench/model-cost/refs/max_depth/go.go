package solution

import "errors"

func MaxDepth(codes []int64) (int64, error) {
	var depth, best int64
	for _, c := range codes {
		switch c {
		case 40:
			depth++
			best = max(best, depth)
		case 41:
			depth--
			if depth < 0 {
				return 0, errors.New("unbalanced")
			}
		}
	}
	if depth != 0 {
		return 0, errors.New("unbalanced")
	}
	return best, nil
}
