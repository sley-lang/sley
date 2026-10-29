package solution

import "errors"

// ParseInt parses a decimal integer from character codes, with an optional
// leading minus sign (45). Digits are codes 48 through 57.
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

	var limit uint64 = 1<<63 - 1
	if neg {
		limit = 1 << 63
	}

	var mag uint64
	for ; i < len(chars); i++ {
		c := chars[i]
		if c < 48 || c > 57 {
			return 0, errors.New("invalid character")
		}
		d := uint64(c - 48)
		if mag > (limit-d)/10 {
			return 0, errors.New("integer overflow")
		}
		mag = mag*10 + d
	}

	if neg {
		return int64(-mag), nil
	}
	return int64(mag), nil
}
