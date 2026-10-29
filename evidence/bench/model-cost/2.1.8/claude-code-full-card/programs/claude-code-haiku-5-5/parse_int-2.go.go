package solution

import "errors"

// ParseInt parses character codes as a decimal integer with an optional
// leading minus sign (code 45). Digits are codes 48 to 57. It returns an
// error if chars is empty, contains only a minus sign, contains any other
// code, or the value does not fit in int64.
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
		return 0, errors.New("no digits")
	}

	// Accumulate as a negative number so that the minimum int64 value is representable.
	const minInt64 = -1 << 63
	var n int64
	for ; i < len(chars); i++ {
		c := chars[i]
		if c < 48 || c > 57 {
			return 0, errors.New("invalid character")
		}
		d := c - 48
		// Check n*10 - d >= minInt64 without overflow.
		if n < (minInt64+d)/10 {
			return 0, errors.New("value out of range")
		}
		n = n*10 - d
	}

	if negative {
		return n, nil
	}
	if n == minInt64 {
		return 0, errors.New("value out of range")
	}
	return -n, nil
}
