package solution

import "errors"

// ParseInt parses a sequence of character codes as a decimal integer.
// An optional leading minus sign (code 45) is allowed; digits are codes 48..57.
// It returns an error if chars is empty, contains only a minus sign, or
// contains any other code.
func ParseInt(chars []int64) (int64, error) {
	if len(chars) == 0 {
		return 0, errors.New("empty input")
	}

	i := 0
	negative := false
	if chars[0] == 45 {
		negative = true
		i = 1
	}

	if i == len(chars) {
		return 0, errors.New("no digits after minus sign")
	}

	var value int64
	for ; i < len(chars); i++ {
		c := chars[i]
		if c < 48 || c > 57 {
			return 0, errors.New("invalid character code")
		}
		value = value*10 + (c - 48)
	}

	if negative {
		value = -value
	}
	return value, nil
}
