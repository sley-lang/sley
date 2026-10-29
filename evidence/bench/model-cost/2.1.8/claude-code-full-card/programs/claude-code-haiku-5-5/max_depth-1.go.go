package solution

import "errors"

// MaxDepth returns the maximum nesting depth of parentheses in codes,
// where 40 is '(' and 41 is ')'. Other codes are ignored. It returns an
// error if the parentheses are unbalanced.
func MaxDepth(codes []int64) (int64, error) {
	var depth, maxDepth int64
	for _, c := range codes {
		switch c {
		case 40:
			depth++
			if depth > maxDepth {
				maxDepth = depth
			}
		case 41:
			if depth == 0 {
				return 0, errors.New("unbalanced parentheses: close without open")
			}
			depth--
		}
	}
	if depth != 0 {
		return 0, errors.New("unbalanced parentheses: open never closed")
	}
	return maxDepth, nil
}
