package solution

import "errors"

func ParseSignedInt(s string) (int64, error) {
	i := 0
	neg := false
	if i < len(s) && (s[i] == '+' || s[i] == '-') {
		neg = s[i] == '-'
		i++
	}
	if i == len(s) {
		return 0, errors.New("invalid integer")
	}
	var v int64
	for ; i < len(s); i++ {
		c := s[i]
		if c < '0' || c > '9' {
			return 0, errors.New("invalid integer")
		}
		v = v*10 + int64(c-'0')
		if v > 1000000 {
			return 0, errors.New("out of range")
		}
	}
	if neg {
		v = -v
	}
	return v, nil
}
