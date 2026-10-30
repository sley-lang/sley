package solution

import (
	"errors"
	"math"
)

// ParseInt parses a sequence of character codes as a decimal integer with an
// optional leading minus sign (code 45). Digits are codes 48 through 57.
// It returns an error if chars is empty, contains only a minus sign, contains
// any other code, or the value does not fit in an int64.
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
			return 0, errors.New("sign without digits")
		}
	}

	// Accumulate as a non-positive value so that math.MinInt64 is representable.
	var n int64 = 0
	for ; i < len(chars); i++ {
		c := chars[i]
		if c < 48 || c > 57 {
			return 0, errors.New("invalid character code")
		}
		d := c - 48
		if n < (math.MinInt64+d)/10 {
			return 0, errors.New("integer overflow")
		}
		n = n*10 - d
	}

	if neg {
		return n, nil
	}
	if n == math.MinInt64 {
		return 0, errors.New("integer overflow")
	}
	return -n, nil
}
