package solution

import (
	"errors"
	"math"
)

// ParseInt parses a sequence of character codes as a decimal int64.
// An optional leading '-' (code 45) is allowed; digits are codes 48..57.
// It returns an error for an empty input, a lone sign, any other code,
// or a value outside the int64 range.
func ParseInt(chars []int64) (int64, error) {
	if len(chars) == 0 {
		return 0, errors.New("empty input")
	}

	i := 0
	neg := false
	if chars[0] == 45 {
		neg = true
		i = 1
	}
	if i == len(chars) {
		return 0, errors.New("no digits")
	}

	const minV int64 = math.MinInt64
	// Accumulate as a negative number to safely cover the full int64 range.
	var n int64 = 0
	for ; i < len(chars); i++ {
		c := chars[i]
		if c < 48 || c > 57 {
			return 0, errors.New("invalid character")
		}
		d := c - 48
		if n < minV/10 {
			return 0, errors.New("integer overflow")
		}
		n10 := n * 10
		if n10 < minV+d {
			return 0, errors.New("integer overflow")
		}
		n = n10 - d
	}

	if neg {
		return n, nil
	}
	if n == minV {
		return 0, errors.New("integer overflow")
	}
	return -n, nil
}
