package solution

import "errors"

// ParseInt parses a decimal integer from character codes, with an optional
// leading minus sign (45). Digits are codes 48 to 57.
func ParseInt(chars []int64) (int64, error) {
	if len(chars) == 0 {
		return 0, errors.New("empty input")
	}

	neg := false
	i := 0
	if chars[0] == 45 {
		neg = true
		i = 1
	}
	if i == len(chars) {
		return 0, errors.New("no digits")
	}

	const minInt64 int64 = -1 << 63

	// Accumulate as a negative number so that math.MinInt64 is representable.
	var n int64
	for ; i < len(chars); i++ {
		c := chars[i]
		if c < 48 || c > 57 {
			return 0, errors.New("invalid character")
		}
		d := c - 48
		if n < minInt64/10 || (n == minInt64/10 && d > 8) {
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
