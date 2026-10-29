package solution

import "errors"

func ParseInt(chars []int64) (int64, error) {
	if len(chars) == 0 {
		return 0, errors.New("empty")
	}
	sign, start := int64(1), 0
	if chars[0] == 45 {
		if len(chars) == 1 {
			return 0, errors.New("lone minus")
		}
		sign, start = -1, 1
	}
	var value int64
	for _, c := range chars[start:] {
		if c < 48 || c > 57 {
			return 0, errors.New("not a digit")
		}
		value = value*10 + (c - 48)
	}
	return sign * value, nil
}
