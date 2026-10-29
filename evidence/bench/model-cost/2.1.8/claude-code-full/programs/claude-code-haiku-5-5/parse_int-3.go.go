package solution

import (
	"errors"
	"math"
)

// ParseInt parses a decimal integer from character codes, with an optional
// leading minus sign (code 45). Digits are codes 48 through 57.
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

	// Magnitude limit: MaxInt64 for positives, |MinInt64| for negatives.
	var limit uint64 = math.MaxInt64
	if neg {
		limit = uint64(math.MaxInt64) + 1
	}

	var n uint64
	for ; i < len(chars); i++ {
		c := chars[i]
		if c < 48 || c > 57 {
			return 0, errors.New("invalid character")
		}
		d := uint64(c - 48)
		if n > (limit-d)/10 {
			return 0, errors.New("integer overflow")
		}
		n = n*10 + d
	}

	if !neg {
		return int64(n), nil
	}
	if n == 0 {
		return 0, nil
	}
	// n is in [1, 2^63]; -(n-1) - 1 avoids overflow at MinInt64.
	return -int64(n-1) - 1, nil
}
