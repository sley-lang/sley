package solution

import "errors"

// ParseInt parses character codes as a decimal integer with an optional
// leading minus sign (code 45). Digits are codes 48 to 57. It returns an
// error for empty input, a lone minus sign, any other code, or a value
// outside the int64 range.
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

	// Magnitude limit: MaxInt64 for positive, MaxInt64+1 for negative.
	var limit uint64 = 1<<63 - 1
	if neg {
		limit = 1 << 63
	}

	var v uint64
	for ; i < len(chars); i++ {
		c := chars[i]
		if c < 48 || c > 57 {
			return 0, errors.New("invalid character code")
		}
		d := uint64(c - 48)
		if v > (limit-d)/10 {
			return 0, errors.New("integer overflow")
		}
		v = v*10 + d
	}

	if neg {
		if v == 1<<63 {
			return -1 << 63, nil
		}
		return -int64(v), nil
	}
	return int64(v), nil
}
