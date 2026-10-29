package solution

import "errors"

// ParseInt parses a decimal integer from character codes, with an optional
// leading minus sign (45). Digits are codes 48 to 57. It returns an error if
// chars is empty, contains only a minus sign, contains any other code, or the
// value overflows int64.
func ParseInt(chars []int64) (int64, error) {
	if len(chars) == 0 {
		return 0, errors.New("empty input")
	}

	neg := false
	i := 0
	if chars[0] == 45 {
		neg = true
		i = 1
		if len(chars) == 1 {
			return 0, errors.New("minus sign without digits")
		}
	}

	// Accumulate as a negative number so that math.MinInt64 is representable.
	const minInt64 = -1 << 63
	var acc int64 = 0
	for ; i < len(chars); i++ {
		c := chars[i]
		if c < 48 || c > 57 {
			return 0, errors.New("invalid character")
		}
		d := c - 48
		// acc*10 - d must be >= minInt64.
		if acc < (minInt64+d)/10 {
			return 0, errors.New("integer overflow")
		}
		acc = acc*10 - d
	}

	if neg {
		return acc, nil
	}
	if acc == minInt64 {
		return 0, errors.New("integer overflow")
	}
	return -acc, nil
}
