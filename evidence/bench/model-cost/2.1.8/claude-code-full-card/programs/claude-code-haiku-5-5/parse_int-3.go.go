package solution

import "errors"

// ParseInt parses character codes as a decimal integer with an optional
// leading minus sign (code 45). Digits are codes 48 to 57.
func ParseInt(chars []int64) (int64, error) {
	if len(chars) == 0 {
		return 0, errors.New("empty input")
	}

	i := 0
	neg := false
	if chars[0] == 45 {
		neg = true
		i = 1
		if len(chars) == 1 {
			return 0, errors.New("minus sign without digits")
		}
	}

	// Accumulate as a negative value so that the minimum int64 is representable.
	const minInt64 = -1 << 63
	var n int64
	for ; i < len(chars); i++ {
		c := chars[i]
		if c < 48 || c > 57 {
			return 0, errors.New("invalid character code")
		}
		d := c - 48
		// Check n*10 - d >= minInt64 without overflowing.
		if n < (minInt64+d)/10 {
			return 0, errors.New("integer overflow")
		}
		n = n*10 - d
	}

	if neg {
		return n, nil
	}
	if n == minInt64 {
		return 0, errors.New("integer overflow")
	}
	return -n, nil
}
