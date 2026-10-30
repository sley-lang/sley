package solution

import "errors"

func ParseClockSeconds(s string) (int64, error) {
	if len(s) != 8 || s[2] != ':' || s[5] != ':' {
		return 0, errors.New("invalid time format")
	}
	field := func(i int) (int64, bool) {
		a, b := s[i], s[i+1]
		if a < '0' || a > '9' || b < '0' || b > '9' {
			return 0, false
		}
		return int64(a-'0')*10 + int64(b-'0'), true
	}
	h, ok1 := field(0)
	m, ok2 := field(3)
	sec, ok3 := field(6)
	if !ok1 || !ok2 || !ok3 {
		return 0, errors.New("invalid time format")
	}
	if h > 23 || m > 59 || sec > 59 {
		return 0, errors.New("time out of range")
	}
	return h*3600 + m*60 + sec, nil
}
