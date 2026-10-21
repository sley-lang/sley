package calc

import "errors"

var (
	ErrEmpty     = errors.New("empty")
	ErrLoneMinus = errors.New("lone minus")
	ErrNotDigit  = errors.New("not a digit")
)

func parseInt(chars []byte) (int64, error) {
	if len(chars) == 0 {
		return 0, ErrEmpty
	}
	sign, start := int64(1), 0
	if chars[0] == '-' {
		if len(chars) == 1 {
			return 0, ErrLoneMinus
		}
		sign, start = -1, 1
	}
	var value int64
	for _, c := range chars[start:] {
		if c < '0' || c > '9' {
			return 0, ErrNotDigit
		}
		value = value*10 + int64(c-'0')
	}
	return sign * value, nil
}
